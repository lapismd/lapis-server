use super::{TrackedTaskError, TrackedTaskProvider};
use crate::services::{
    Error, TaskObservationWrite, Work,
    daily::{self, DailyDocumentPlan, DefaultTaskObservationError},
    operations,
};
use lapis_app_protocol::{ErrorCode, ExpectedVersion};
use lapis_task_services::{
    ParsedTaskDocument, TaskOccurrenceMaterialization, TaskPlan, TaskTracking,
    task_occurrence_day_part,
};
#[cfg(test)]
mod tests;

type Result<T> = std::result::Result<T, TrackedTaskError>;
pub(super) struct Applied {
    pub write: TaskObservationWrite,
    pub changed: bool,
}
fn source(work: &Work, doc: &ParsedTaskDocument) -> Result<()> {
    let current = work.read_text(&doc.path)?;
    if current.text != doc.raw {
        return Err(Error::new(
            ErrorCode::Conflict,
            "Tracked Task source changed before admission",
        )
        .into());
    }
    let state = work.vault.lock()?;
    operations::guard_existing_write(
        &state,
        &doc.path,
        &ExpectedVersion::Version {
            version: current.version,
        },
        None,
    )?;
    Ok(())
}
fn normalized<T: serde::de::DeserializeOwned>(value: &serde_json::Value) -> Result<Option<T>> {
    if value.is_null() {
        return Ok(None);
    }
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|_| {
            Error::new(
                ErrorCode::InvalidRequest,
                "Invalid normalized tracked Task input",
            )
            .into()
        })
}
pub(super) fn apply(
    work: &Work,
    doc: &ParsedTaskDocument,
    date: &str,
    provider: &TrackedTaskProvider,
) -> Result<Applied> {
    apply_with(work, doc, date, provider, |_| {})
}
fn apply_with(
    work: &Work,
    doc: &ParsedTaskDocument,
    date: &str,
    provider: &TrackedTaskProvider,
    after_ensure: impl FnOnce(&lapis_app_protocol::TextSnapshot),
) -> Result<Applied> {
    let _gate = work
        .vault
        .task_lifecycle
        .lock()
        .map_err(|_| Error::busy())?;
    source(work, doc)?;
    let policy = match provider {
        TrackedTaskProvider::Default(policy) => policy,
        TrackedTaskProvider::Unavailable(error) => {
            return Err(TrackedTaskError::Provider(error.clone()));
        }
    };
    let task = doc
        .task
        .as_ref()
        .ok_or_else(|| Error::new(ErrorCode::InvalidRequest, "Tracked source is not a Task"))?;
    let plan: Option<TaskPlan> = normalized(&task["plan"])?;
    let tracking: Option<TaskTracking> = normalized(&task["tracking"])?;
    let input = TaskOccurrenceMaterialization {
        task_id: doc.id.clone(),
        task_path: doc.path.clone(),
        title: doc.title.clone(),
        occurrence_date: date.into(),
        tracking,
        day_part: task_occurrence_day_part(plan.as_ref(), &policy.day_parts),
    };
    // Reuse the existing daily snapshot and writer. Its physical verification
    // also fences source changes between this preflight and the first mutation.
    let prepared = daily::io::prepare(work, date, Some((&policy.daily, &policy.format)))
        .map_err(|error| TrackedTaskError::Daily(DefaultTaskObservationError::Provider(error)))?;
    source(work, doc)?;
    let before = match prepared.plan() {
        Some(DailyDocumentPlan::Existing { path }) => Some(prepared.existing(path)?),
        _ => None,
    };
    let snapshot = daily::io::finish(work, prepared)
        .map_err(|error| TrackedTaskError::Daily(DefaultTaskObservationError::Provider(error)))?
        .ok_or_else(|| {
            Error::new(
                ErrorCode::Io,
                "Default daily provider did not resolve a document",
            )
        })?;
    after_ensure(&snapshot);
    let write = work
        .process_task_occurrence(
            &snapshot.path,
            &ExpectedVersion::Version {
                version: snapshot.version,
            },
            &input,
            None,
        )
        .map_err(|error| TrackedTaskError::Daily(DefaultTaskObservationError::Write(error)))?;
    let changed = before.is_none_or(|before| before.text != write.document.text);
    Ok(Applied { write, changed })
}
