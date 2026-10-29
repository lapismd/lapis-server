use super::unavailable;
use crate::services::{
    Result, TaskLifecycleDisposition, TaskLifecyclePolicy, TaskLifecycleStatus, TrackedTaskStatus,
};
use lapis_app_protocol::native_tasks::*;
use serde::{Serialize, de::DeserializeOwned};

pub(super) fn input<T: DeserializeOwned>(value: impl Serialize) -> Result<T> {
    serde_json::to_value(value)
        .and_then(serde_json::from_value)
        .map_err(|_| {
            crate::services::Error::new(
                lapis_app_protocol::ErrorCode::InvalidRequest,
                "Invalid native Task value",
            )
        })
}
pub(super) fn output<T: DeserializeOwned>(value: impl Serialize) -> Result<T> {
    serde_json::to_value(value)
        .and_then(serde_json::from_value)
        .map_err(|_| unavailable())
}
pub(super) fn policy(value: &NativeTaskConfiguration) -> Result<TaskLifecyclePolicy> {
    Ok(TaskLifecyclePolicy {
        day_parts: input(&value.day_parts)?,
        daily: input(&value.daily)?,
        format: input(&value.format)?,
    })
}
pub(super) fn operation(value: TaskLifecycleStatus) -> Result<NativeTaskOperation> {
    Ok(NativeTaskOperation {
        request: output(value.request)?,
        disposition: match value.disposition {
            TaskLifecycleDisposition::Retryable => NativeTaskDisposition::Retryable,
            TaskLifecycleDisposition::Divergent => NativeTaskDisposition::Divergent,
            TaskLifecycleDisposition::Completed => NativeTaskDisposition::Completed,
        },
        original: value.original,
        task_replacement: value.task_replacement,
        daily: value.daily,
        daily_replacement: value.daily_replacement,
        last_failure: value.last_failure.map(output).transpose()?,
    })
}
pub(super) fn tracked(value: TrackedTaskStatus) -> NativeTaskTrackedStatus {
    let omitted_failures = value.failures.len().saturating_sub(100);
    NativeTaskTrackedStatus {
        date: value.date,
        attempted: value.attempted,
        materialized: value.materialized,
        changed: value.changed,
        failures: value
            .failures
            .into_iter()
            .take(100)
            .map(|failure| NativeTaskTrackedFailure {
                path: failure.path,
                message: failure.cause.to_string().chars().take(2048).collect(),
                daily_path: failure
                    .retained_daily
                    .as_ref()
                    .map(|value| value.path.clone()),
                daily_version: failure.retained_daily.map(|value| value.version),
            })
            .collect(),
        omitted_failures,
    }
}
