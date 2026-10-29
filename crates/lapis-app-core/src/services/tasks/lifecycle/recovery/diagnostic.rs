use super::{TaskLifecycleFailure, TaskRecoveryError};
use crate::services::{Error, TaskWriteError, daily::DailyDocumentOperationError};

fn service(error: &Error) -> String {
    serde_json::to_value(error.code())
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "service".into())
}
/// A journal/owner loss is uncertain, rather than an acknowledged domain failure.
pub(super) fn known(error: &TaskRecoveryError) -> Option<TaskLifecycleFailure> {
    let code = match error {
        TaskRecoveryError::Service(error)
        | TaskRecoveryError::Task(TaskWriteError::Service(error)) => service(error),
        TaskRecoveryError::Task(TaskWriteError::Transform(error)) => error
            .code
            .clone()
            .unwrap_or_else(|| "task-transform".into()),
        TaskRecoveryError::Daily(DailyDocumentOperationError::Service(error)) => service(error),
        TaskRecoveryError::Daily(_) => "daily-provider".into(),
        TaskRecoveryError::Journal(_) | TaskRecoveryError::Partial { .. } => return None,
    };
    let mut message = String::new();
    for ch in error.to_string().chars() {
        if message.len() + ch.len_utf8() > 2048 {
            break;
        }
        message.push(if ch == '\0' { ' ' } else { ch });
    }
    let code = if code.is_empty() || code.len() > 128 || code.chars().any(char::is_control) {
        "task-failure".into()
    } else {
        code
    };
    if message.trim().is_empty() {
        message = "Task lifecycle operation failed".into();
    }
    Some(TaskLifecycleFailure { code, message })
}
