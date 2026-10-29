//! Native-only ordered persistence. Durable retry and host selection remain gated.
mod error;
mod recovery;
pub use recovery::{
    TaskLifecycleDisposition, TaskLifecycleFailure, TaskLifecycleJournal, TaskLifecyclePolicy,
    TaskLifecycleRequest, TaskLifecycleStatus, TaskRecoveryError,
};
#[cfg(test)]
mod tests;

use super::TaskWriteResult;
use crate::services::daily::{DailyDocumentSettings, DailyFormatContext};
use crate::services::{Error, TaskObservationWrite, Work, operations};
pub use error::{TaskLifecycleError, TaskLifecyclePartial};
use lapis_app_protocol::{ExpectedVersion, TextSnapshot, VaultPhase};
use lapis_task_services::{
    TaskDayPartSettings, TaskLifecycleAction, TaskLifecyclePlan, plan_task_lifecycle,
};

/// One admitted operation; private fields prevent replacing the original plan.
pub struct PreparedTaskLifecycle {
    work: Work,
    original: TextSnapshot,
    action: TaskLifecycleAction,
    plan: TaskLifecyclePlan,
}

#[derive(Debug)]
pub struct TaskLifecycleWrite {
    pub task: TextSnapshot,
    pub daily: Option<TaskObservationWrite>,
}

pub(super) fn original(
    work: &Work,
    path: &str,
    expected: &ExpectedVersion,
) -> TaskWriteResult<TextSnapshot> {
    let state = work.vault.lock()?;
    if state.phase == VaultPhase::Closed {
        return Err(Error::closed().into());
    }
    Ok(operations::guard_existing_write(
        &state, path, expected, None,
    )?)
}

impl Work {
    pub fn prepare_task_lifecycle(
        self,
        path: &str,
        expected: &ExpectedVersion,
        action: TaskLifecycleAction,
        settings: &TaskDayPartSettings,
    ) -> TaskWriteResult<PreparedTaskLifecycle> {
        let original = original(&self, path, expected)?;
        let plan = plan_task_lifecycle(&original.text, path, &action, settings)?;
        Ok(PreparedTaskLifecycle {
            work: self,
            original,
            action,
            plan,
        })
    }
}

impl PreparedTaskLifecycle {
    pub fn persist_default_daily(
        self,
        settings: &DailyDocumentSettings,
        context: &DailyFormatContext,
    ) -> Result<TaskLifecycleWrite, TaskLifecycleError> {
        persist(self, settings, context, || {})
    }
}

fn persist(
    prepared: PreparedTaskLifecycle,
    settings: &DailyDocumentSettings,
    context: &DailyFormatContext,
    after_daily: impl FnOnce(),
) -> Result<TaskLifecycleWrite, TaskLifecycleError> {
    let PreparedTaskLifecycle {
        work,
        original: before,
        action,
        plan,
    } = prepared;
    let vault = work.vault.clone();
    let _gate = vault
        .task_lifecycle
        .lock()
        .map_err(|_| Error::busy())
        .map_err(|error| TaskLifecycleError::Admission(error.into()))?;
    let expected = ExpectedVersion::Version {
        version: before.version.clone(),
    };
    original(&work, &before.path, &expected).map_err(TaskLifecycleError::Admission)?;
    let daily = plan
        .observation
        .as_ref()
        .map(|observation| {
            work.process_default_daily_task_occurrence(
                settings,
                context,
                &observation.input,
                observation.completion.as_ref(),
            )
        })
        .transpose()
        .map_err(TaskLifecycleError::Daily)?;
    if daily.is_some() {
        after_daily();
    }
    let written = super::persistence::apply(&work, &before.path, &expected, |_| {
        Ok(plan.task_raw.clone())
    });
    match written {
        Ok(task) => Ok(TaskLifecycleWrite { task, daily }),
        Err(cause) => Err(match daily {
            None => TaskLifecycleError::Admission(cause),
            Some(daily) => TaskLifecycleError::Partial(Box::new(TaskLifecyclePartial {
                original: before,
                action,
                task_replacement: plan.task_raw,
                daily,
                cause,
            })),
        }),
    }
}
