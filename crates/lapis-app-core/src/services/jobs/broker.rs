use super::*;
use lapis_app_protocol::{
    ExpectedVersion,
    database::{
        call::*,
        indexed::queries::MetadataPageQuery,
        state::{NotificationRecord, NotificationSeverity},
    },
};
use lapis_plugin_services::markdown::{patch_frontmatter, read_frontmatter};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(super) struct Broker {
    pub owner: Arc<Owner>,
    pub work: Work,
    pub index: usize,
    pub uncertain: AtomicBool,
    pub last_error: Mutex<Option<String>>,
}

impl JobHost for Broker {
    fn cancelled(&self) -> bool {
        self.owner.stopped.load(Ordering::Acquire)
            || self.owner.cancelled.load(Ordering::Acquire)
            || self.work.cancellation_requested()
    }
    fn invoke(&self, action: &str, input: Value) -> std::result::Result<Value, String> {
        self.call(action, input).map_err(|error| {
            let message = error.to_string();
            if let Ok(mut last) = self.last_error.lock() {
                *last = Some(message.clone());
            }
            message
        })
    }
}

impl Broker {
    fn call(&self, action: &str, input: Value) -> Result<Value> {
        let mut stored = self.owner.store()?;
        if self.cancelled() || !stored.catalog.trusted || stored.catalog.safe_mode {
            return Err(Error::denied());
        }
        let job = stored
            .catalog
            .jobs
            .get(self.index)
            .ok_or_else(|| invalid("Job configuration disappeared"))?
            .clone();
        if !job.configuration.enabled || job.transferred_to.is_some() {
            return Err(Error::denied());
        }
        discovery::source(&self.work.vault, &job)?;
        let permissions = &job.configuration.permissions;
        match action {
            "cancelled" => Ok(json!(false)),
            "state.load" => Ok(job.state),
            "state.save" => {
                if serde_json::to_vec(&input)
                    .map_err(|_| invalid("Invalid state"))?
                    .len()
                    > 65536
                {
                    return Err(invalid("Job state exceeds 64 KiB"));
                }
                stored.update(|catalog| {
                    catalog.jobs[self.index].state = input;
                    Ok(())
                })?;
                Ok(Value::Null)
            }
            "metadata.query" if permissions.metadata_read => {
                let mut query: MetadataPageQuery =
                    serde_json::from_value(input).map_err(|_| invalid("Invalid metadata query"))?;
                if query.limit.is_some_and(|limit| limit == 0 || limit > 100) {
                    return Err(invalid("Job metadata pages are limited to 100 rows"));
                }
                query.limit = Some(query.limit.unwrap_or(100));
                let filter = query.query.get_or_insert_with(Default::default);
                filter.exclude_hidden_paths = true;
                filter.extensions = vec!["md".into()];
                #[cfg(feature = "local-sibling-projections")]
                {
                    self.owner.executor.handle.block_on(async {
                        self.work.vault.start_markdown_indexing().await?;
                        if query.after.is_none() {
                            self.work.vault.refresh_indexing(false).await?;
                        }
                        let database = self.work.vault.database().await?;
                        let page = database.query_indexed_metadata_page(query).await?;
                        serde_json::to_value(page).map_err(|_| invalid("Invalid metadata response"))
                    })
                }
                #[cfg(not(feature = "local-sibling-projections"))]
                {
                    Err(Error::new(
                        ErrorCode::Unsupported,
                        "Native metadata indexing is unavailable",
                    ))
                }
            }
            "notes.frontmatter" if permissions.frontmatter_read => {
                let path = note_path(&input)?;
                let snapshot = self.work.read_text(path)?;
                if snapshot.text.len() > 1024 * 1024 {
                    return Err(invalid("Note exceeds the background read limit"));
                }
                let properties = read_frontmatter(&snapshot.text)
                    .map_err(|_| invalid("Unsupported note frontmatter"))?;
                Ok(json!({"path":path, "version":snapshot.version, "properties":properties}))
            }
            "notes.patchFrontmatter" if !permissions.frontmatter_fields.is_empty() => {
                let path = note_path(&input)?;
                let set = input["set"].as_object().cloned().unwrap_or_default();
                let remove: Vec<String> = serde_json::from_value(
                    input.get("remove").cloned().unwrap_or_else(|| json!([])),
                )
                .map_err(|_| invalid("Invalid frontmatter removal"))?;
                if set.is_empty() && remove.is_empty()
                    || set
                        .keys()
                        .chain(remove.iter())
                        .any(|key| !permissions.frontmatter_fields.contains(key))
                {
                    return Err(Error::denied());
                }
                let expected = ExpectedVersion::Version {
                    version: input["expectedVersion"]
                        .as_str()
                        .ok_or_else(|| invalid("Note version is required"))?
                        .into(),
                };
                let state = self.work.vault.lock()?;
                state.require_open()?;
                let snapshot =
                    super::super::operations::guard_existing_write(&state, path, &expected, None)?;
                if snapshot.text.len() > 1024 * 1024 {
                    return Err(invalid("Note exceeds the background write limit"));
                }
                let text = patch_frontmatter(&snapshot.text, &set, &remove)
                    .map_err(|_| invalid("Unsupported frontmatter patch"))?;
                self.uncertain.store(true, Ordering::Release);
                let version = if snapshot.text == text {
                    snapshot.version
                } else {
                    super::super::operations::write(&state, path, &text, &expected, None)?
                };
                self.uncertain.store(false, Ordering::Release);
                Ok(json!({"path":path, "version":version}))
            }
            "notifications.notify" if permissions.notifications => {
                let key = input["id"]
                    .as_str()
                    .filter(|key| !key.is_empty() && key.len() <= 128)
                    .ok_or_else(|| invalid("Stable notification identity is required"))?;
                let title = input["title"].as_str().unwrap_or(&job.descriptor.title);
                let message = input["message"]
                    .as_str()
                    .filter(|value| !value.is_empty() && value.len() <= 4096)
                    .ok_or_else(|| invalid("Notification message exceeds bounds"))?;
                if title.len() > 256 {
                    return Err(invalid("Notification title exceeds bounds"));
                }
                if job.notifications.iter().any(|existing| existing == key) {
                    return Ok(Value::Null);
                }
                // Reserve before delivery. A lost process cannot emit this identity twice.
                stored.update(|catalog| {
                    let keys = &mut catalog.jobs[self.index].notifications;
                    if keys.len() >= 128 {
                        keys.remove(0);
                    }
                    keys.push(key.into());
                    Ok(())
                })?;
                let id = format!(
                    "plugin-job-{:x}",
                    Sha256::digest(format!("{}/{}/{}", job.plugin, job.descriptor.id, key))
                );
                let record = NotificationRecord {
                    id,
                    title: Some(title.into()),
                    message: message.into(),
                    severity: NotificationSeverity::Info,
                    source: Some(job.plugin),
                    created_at: now() as f64,
                    updated_at: now() as f64,
                    read: false,
                    cleared: false,
                };
                self.uncertain.store(true, Ordering::Release);
                self.owner
                    .executor
                    .handle
                    .block_on(self.work.call_database(DatabaseCall {
                        method: DatabaseMethod::UpsertNotification,
                        args: vec![DatabaseValue::Value {
                            value: serde_json::to_value(&record)
                                .map_err(|_| invalid("Invalid notification"))?,
                        }],
                    }))?;
                self.uncertain.store(false, Ordering::Release);
                if job.configuration.native_notifications {
                    let deliver = self.owner.notification_delivery.as_ref().ok_or_else(|| {
                        Error::new(
                            ErrorCode::Unsupported,
                            "Native notifications are unavailable on this host",
                        )
                    })?;
                    deliver(&record).map_err(|_| Error::new(ErrorCode::Io, "Native notification delivery failed. Review OS permissions; Lapis history retains the notification."))?;
                }
                Ok(Value::Null)
            }
            _ => Err(Error::denied()),
        }
    }
}

fn note_path(input: &Value) -> Result<&str> {
    let path = input["path"]
        .as_str()
        .ok_or_else(|| invalid("Note path is required"))?;
    lapis_app_protocol::content::validate_content_path(path)
        .map_err(|_| invalid("Invalid note path"))?;
    if !path.ends_with(".md") || path.split('/').any(|part| part.starts_with('.')) {
        return Err(Error::denied());
    }
    Ok(path)
}
