use super::{errors, projection};
use crate::services::{Error, TaskLifecyclePolicy, TaskWriteError, Work};
use lapis_app_protocol::{ErrorCode, ExpectedVersion, native_tasks::*};
use lapis_task_services::{
    TaskOccurrenceMaterialization, TaskPlan, TaskTracking, task_occurrence_day_part,
};

pub(super) fn apply(
    work: &Work,
    path: &str,
    expected_version: &str,
    date: &str,
    completion: Option<NativeTaskCompletion>,
    policy: &TaskLifecyclePolicy,
) -> NativeTaskValue {
    let result = (|| {
        let _gate = work
            .vault
            .task_lifecycle
            .lock()
            .map_err(|_| TaskWriteError::from(Error::busy()))?;
        let original = super::super::lifecycle::original(
            work,
            path,
            &ExpectedVersion::Version {
                version: expected_version.into(),
            },
        )?;
        let parsed =
            lapis_task_services::parse_task_document(&original.text, path).ok_or_else(|| {
                TaskWriteError::from(Error::new(
                    ErrorCode::InvalidRequest,
                    "Source is not a Task document",
                ))
            })?;
        let task = parsed.task.ok_or_else(|| {
            TaskWriteError::from(Error::new(
                ErrorCode::InvalidRequest,
                "Source is not a Task",
            ))
        })?;
        let plan: Option<TaskPlan> =
            projection::input(task["plan"].clone()).map_err(TaskWriteError::from)?;
        let tracking: Option<TaskTracking> =
            projection::input(task["tracking"].clone()).map_err(TaskWriteError::from)?;
        let input = TaskOccurrenceMaterialization {
            task_id: parsed.id,
            task_path: path.into(),
            title: parsed.title,
            occurrence_date: date.into(),
            tracking,
            day_part: task_occurrence_day_part(plan.as_ref(), &policy.day_parts),
        };
        let completion = completion
            .map(projection::input)
            .transpose()
            .map_err(TaskWriteError::from)?;
        work.process_default_daily_task_occurrence(
            &policy.daily,
            &policy.format,
            &input,
            completion.as_ref(),
        )
    })();
    match result {
        Ok(write) => match projection::output(write.observation) {
            Ok(observation) => NativeTaskValue::Observation {
                document: write.document,
                observation: Box::new(observation),
            },
            Err(error) => errors::value(errors::service(error)),
        },
        Err(error) => errors::value(errors::observation(error)),
    }
}
