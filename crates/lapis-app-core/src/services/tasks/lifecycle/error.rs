use crate::services::{TaskObservationWrite, TaskWriteError, daily::DefaultTaskObservationError};
use lapis_app_protocol::TextSnapshot;
use lapis_task_services::TaskLifecycleAction;
use std::fmt;

#[derive(Debug)]
pub struct TaskLifecyclePartial {
    pub original: TextSnapshot,
    pub action: TaskLifecycleAction,
    pub task_replacement: String,
    pub daily: TaskObservationWrite,
    pub cause: TaskWriteError,
}

#[derive(Debug)]
pub enum TaskLifecycleError {
    Admission(TaskWriteError),
    Daily(DefaultTaskObservationError),
    Partial(Box<TaskLifecyclePartial>),
}

impl fmt::Display for TaskLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => error.fmt(formatter),
            Self::Daily(error) => error.fmt(formatter),
            Self::Partial(partial) => write!(
                formatter,
                "Daily observation persisted; original Task persistence failed: {}",
                partial.cause
            ),
        }
    }
}
impl std::error::Error for TaskLifecycleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Admission(error) => error,
            Self::Daily(error) => error,
            Self::Partial(partial) => &partial.cause,
        })
    }
}
