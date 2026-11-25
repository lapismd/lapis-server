use super::*;
use storage::{Catalog, StoredJob};

pub(super) fn find<'a>(
    catalog: &'a mut Catalog,
    plugin: &str,
    id: &str,
) -> Result<&'a mut StoredJob> {
    catalog
        .jobs
        .iter_mut()
        .find(|job| job.plugin == plugin && job.descriptor.id == id)
        .ok_or_else(|| invalid("Background job is not configured"))
}
pub(super) fn active(job: &StoredJob) -> bool {
    job.runs
        .last()
        .is_some_and(|run| run.phase == JobPhase::Running)
}
pub(super) fn permissions(granted: &JobPermissions, declared: &JobPermissions) -> bool {
    (!granted.metadata_read || declared.metadata_read)
        && (!granted.frontmatter_read || declared.frontmatter_read)
        && (!granted.notifications || declared.notifications)
        && granted
            .frontmatter_fields
            .iter()
            .all(|field| declared.frontmatter_fields.contains(field))
}

impl Client {
    pub fn plugin_jobs(&self, command: JobCommand) -> Result<JobSnapshot> {
        self.vault.lock()?.require_client(self.id)?;
        let owner = self.vault.jobs()?;
        if owner.stopped.load(Ordering::Acquire) {
            return Err(Error::closed());
        }
        let mut transfer = None;
        match command {
            JobCommand::List {} => {}
            JobCommand::Configure {
                plugin_id,
                job_id,
                configuration,
            } => {
                let installed = discovery::discover(&self.vault)?
                    .into_iter()
                    .find(|job| job.plugin == plugin_id && job.descriptor.id == job_id)
                    .ok_or_else(|| invalid("Background job is unavailable"))?;
                configuration
                    .schedule
                    .validate()
                    .map_err(|message| invalid(&message))?;
                if configuration.fingerprint != installed.fingerprint
                    || !permissions(
                        &configuration.permissions,
                        &installed.descriptor.permissions,
                    )
                {
                    return Err(Error::denied());
                }
                if configuration.native_notifications && owner.notification_delivery.is_none() {
                    return Err(Error::new(
                        ErrorCode::Unsupported,
                        "Native notifications are unavailable on this host",
                    ));
                }
                let mut stored = owner.store()?;
                stored.update(|catalog| {
                    if !catalog.trusted || catalog.safe_mode {
                        return Err(Error::denied());
                    }
                    if let Some(job) = catalog
                        .jobs
                        .iter_mut()
                        .find(|job| job.plugin == plugin_id && job.descriptor.id == job_id)
                    {
                        if active(job)
                            || job
                                .runs
                                .iter()
                                .any(|run| run.phase == JobPhase::Interrupted)
                        {
                            return Err(Error::busy());
                        }
                        if job.transferred_to.is_some() {
                            return Err(invalid(
                                "Import an explicit return handoff before enabling this job",
                            ));
                        }
                        job.descriptor = installed.descriptor.clone();
                        job.last_scheduled_at = job.last_scheduled_at.max(
                            configuration
                                .schedule
                                .latest(now())
                                .map_err(|message| invalid(&message))?
                                .scheduled_at,
                        );
                        job.last_occurrence = Some(
                            configuration
                                .schedule
                                .latest(now())
                                .map_err(|message| invalid(&message))?
                                .key,
                        );
                        job.configuration = configuration;
                    } else {
                        if catalog.jobs.len() >= 64 {
                            return Err(invalid("Background job limit reached"));
                        }
                        catalog.jobs.push(StoredJob {
                            plugin: plugin_id,
                            descriptor: installed.descriptor,
                            last_occurrence: Some(
                                configuration
                                    .schedule
                                    .latest(now())
                                    .map_err(|message| invalid(&message))?
                                    .key,
                            ),
                            last_scheduled_at: configuration
                                .schedule
                                .latest(now())
                                .map_err(|message| invalid(&message))?
                                .scheduled_at,
                            configuration,
                            state: serde_json::Value::Null,
                            runs: Vec::new(),
                            requested: false,
                            transferred_to: None,
                            notifications: Vec::new(),
                            release: None,
                        });
                    }
                    Ok(())
                })?;
            }
            JobCommand::Pause { plugin_id, job_id } => {
                let mut stored = owner.store()?;
                let job = find(&mut stored.catalog.clone(), &plugin_id, &job_id)?.clone();
                if active(&job) {
                    owner.cancelled.store(true, Ordering::Release);
                }
                stored.update(|catalog| {
                    let job = find(catalog, &plugin_id, &job_id)?;
                    job.configuration.enabled = false;
                    job.requested = false;
                    Ok(())
                })?;
            }
            JobCommand::RunNow { plugin_id, job_id } => {
                let job = find(&mut owner.store()?.catalog.clone(), &plugin_id, &job_id)?.clone();
                discovery::source(&self.vault, &job)?;
                owner.store()?.update(|catalog| {
                    if !catalog.trusted || catalog.safe_mode {
                        return Err(Error::denied());
                    }
                    let job = find(catalog, &plugin_id, &job_id)?;
                    if !job.configuration.enabled || job.transferred_to.is_some() {
                        return Err(Error::denied());
                    }
                    if !active(job)
                        && !job
                            .runs
                            .iter()
                            .any(|run| run.phase == JobPhase::Interrupted)
                    {
                        job.requested = true;
                    }
                    Ok(())
                })?;
            }
            JobCommand::Resolve {
                plugin_id,
                job_id,
                run_id,
            } => {
                owner.store()?.update(|catalog| {
                    let job = find(catalog, &plugin_id, &job_id)?;
                    let run = job
                        .runs
                        .iter_mut()
                        .find(|run| run.id == run_id && run.phase == JobPhase::Interrupted)
                        .ok_or_else(|| invalid("Interrupted run identity changed"))?;
                    run.phase = JobPhase::Failed;
                    run.error = Some(
                        "Owner acknowledged the interrupted outcome; invocation was not replayed"
                            .into(),
                    );
                    run.finished_at = Some(now());
                    Ok(())
                })?;
            }
            JobCommand::Fence { trusted, safe_mode } => {
                if !trusted || safe_mode {
                    owner.cancelled.store(true, Ordering::Release);
                }
                owner.store()?.update(|catalog| {
                    catalog.trusted = trusted;
                    catalog.safe_mode = safe_mode;
                    Ok(())
                })?;
            }
            JobCommand::Transfer {
                plugin_id,
                job_id,
                to_runner,
                to_vault_id,
            } => {
                transfer = Some(super::handoff::release(
                    &owner,
                    &plugin_id,
                    &job_id,
                    &to_runner,
                    &to_vault_id,
                )?);
            }
            JobCommand::ImportTransfer { receipt } => {
                super::handoff::accept(&self.vault, &owner, *receipt)?
            }
        }
        owner.changed.notify_all();
        snapshot::snapshot(&self.vault, &owner, transfer)
    }
}
