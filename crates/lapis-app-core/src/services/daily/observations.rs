use super::{DailyDocumentSettings, DailyFormatContext, DefaultTaskObservationError};
use crate::services::{TaskObservationWrite, Work};
use lapis_app_protocol::ExpectedVersion;
use lapis_task_services::{TaskOccurrenceCompletion, TaskOccurrenceMaterialization};

impl Work {
    /// Explicit default provider only; this does not advance the original Task.
    pub fn process_default_daily_task_occurrence(
        &self,
        settings: &DailyDocumentSettings,
        context: &DailyFormatContext,
        input: &TaskOccurrenceMaterialization,
        completion: Option<&TaskOccurrenceCompletion>,
    ) -> Result<TaskObservationWrite, DefaultTaskObservationError> {
        let snapshot =
            self.ensure_default_daily_document(&input.occurrence_date, settings, context)?;
        Ok(self.process_task_occurrence(
            &snapshot.path,
            &ExpectedVersion::Version {
                version: snapshot.version,
            },
            input,
            completion,
        )?)
    }
}
