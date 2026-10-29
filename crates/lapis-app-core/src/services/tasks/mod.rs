//! Explicit one-file transformations using the admitted vault's existing writer.
mod error;
mod lifecycle;
mod observations;
mod persistence;
pub(in crate::services) mod service;
pub use service::TaskStorage;
#[cfg(test)]
mod tests;

use super::Work;
pub use error::{TaskWriteError, TaskWriteResult};
use lapis_app_protocol::{ExpectedVersion, TextSnapshot};
use lapis_task_services::{
    TaskMutation, TaskOccurrenceCompletion, TaskOccurrenceMaterialization,
    TaskOccurrenceObservation,
};
pub use lifecycle::{
    PreparedTaskLifecycle, TaskLifecycleDisposition, TaskLifecycleError, TaskLifecycleFailure,
    TaskLifecycleJournal, TaskLifecyclePartial, TaskLifecyclePolicy, TaskLifecycleRequest,
    TaskLifecycleStatus, TaskLifecycleWrite, TaskRecoveryError,
};

#[derive(Debug)]
pub struct TaskObservationWrite {
    pub document: TextSnapshot,
    pub observation: TaskOccurrenceObservation,
}

impl Work {
    pub fn process_task_occurrence(
        &self,
        path: &str,
        expected: &ExpectedVersion,
        input: &TaskOccurrenceMaterialization,
        completion: Option<&TaskOccurrenceCompletion>,
    ) -> TaskWriteResult<TaskObservationWrite> {
        let (document, observation) = persistence::apply_with(self, path, expected, |raw| {
            observations::process(raw, path, input, completion)
        })?;
        Ok(TaskObservationWrite {
            document,
            observation,
        })
    }

    pub fn mutate_task(
        &self,
        path: &str,
        expected: &ExpectedVersion,
        mutation: &TaskMutation,
    ) -> TaskWriteResult<TextSnapshot> {
        persistence::apply(self, path, expected, |raw| {
            lapis_task_services::apply_task_mutation(raw, path, mutation)
        })
    }

    pub fn materialize_task_occurrence(
        &self,
        path: &str,
        expected: &ExpectedVersion,
        input: &TaskOccurrenceMaterialization,
    ) -> TaskWriteResult<TextSnapshot> {
        persistence::apply(self, path, expected, |raw| {
            lapis_task_services::materialize_task_occurrence(raw, path, input)
        })
    }

    pub fn complete_task_occurrence(
        &self,
        path: &str,
        expected: &ExpectedVersion,
        observation: &TaskOccurrenceObservation,
        completion: &TaskOccurrenceCompletion,
    ) -> TaskWriteResult<TextSnapshot> {
        persistence::apply(self, path, expected, |raw| {
            lapis_task_services::complete_task_occurrence(raw, path, observation, completion)
        })
    }
}
