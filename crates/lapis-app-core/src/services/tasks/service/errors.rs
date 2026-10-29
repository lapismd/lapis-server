use super::projection;
use crate::services::{
    Error, TaskRecoveryError, TaskWriteError,
    daily::{DailyDocumentOperationError, DefaultTaskObservationError},
};
use lapis_app_protocol::native_tasks::*;
use uuid::Uuid;

fn failure(
    name: String,
    code: Option<String>,
    message: String,
    parts: Option<Vec<String>>,
    uncertain: bool,
) -> NativeTaskFailure {
    NativeTaskFailure {
        name,
        code,
        message,
        parts,
        request_id: None,
        uncertain,
        operation: None,
    }
}
pub(super) fn service(error: Error) -> NativeTaskFailure {
    let code = serde_json::to_value(error.code())
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned));
    let uncertain = matches!(error.code(), lapis_app_protocol::ErrorCode::Io);
    failure("Error".into(), code, error.to_string(), None, uncertain)
}
pub(super) fn write(error: TaskWriteError) -> NativeTaskFailure {
    match error {
        TaskWriteError::Service(error) => service(error),
        TaskWriteError::Transform(error) => {
            failure(error.name, error.code, error.message, error.parts, false)
        }
    }
}
pub(super) fn daily(error: DailyDocumentOperationError) -> NativeTaskFailure {
    match error {
        DailyDocumentOperationError::Service(error) => service(error),
        DailyDocumentOperationError::Policy(error) => provider(error.message),
    }
}
pub(super) fn observation(error: DefaultTaskObservationError) -> NativeTaskFailure {
    match error {
        DefaultTaskObservationError::Provider(error) => daily(error),
        DefaultTaskObservationError::Write(error) => write(error),
    }
}
pub(super) fn provider(message: String) -> NativeTaskFailure {
    failure(
        "Error".into(),
        Some("DAILY_PROVIDER_UNAVAILABLE".into()),
        message,
        None,
        false,
    )
}
pub(super) fn recovery(error: TaskRecoveryError, id: Uuid) -> NativeTaskFailure {
    let mut value = match error {
        TaskRecoveryError::Service(error) => service(error),
        TaskRecoveryError::Task(error) => write(error),
        TaskRecoveryError::Daily(error) => daily(error),
        TaskRecoveryError::Journal(_) => failure(
            "Error".into(),
            Some("TASK_OUTCOME_UNKNOWN".into()),
            "Native Task journal outcome is unavailable; inspect before retrying".into(),
            None,
            true,
        ),
        TaskRecoveryError::Partial { status, cause } => {
            let mut value = recovery(*cause, id);
            value.operation = projection::operation(*status).ok().map(Box::new);
            value
        }
    };
    value.request_id = Some(id);
    value
}
pub(super) fn value(failure: NativeTaskFailure) -> NativeTaskValue {
    NativeTaskValue::Failure {
        failure: Box::new(failure),
    }
}
