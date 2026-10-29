use crate::services::Error;
use lapis_task_services::TaskTransformError;
use std::fmt;

#[derive(Debug)]
pub enum TaskWriteError {
    Service(Error),
    Transform(TaskTransformError),
}

pub type TaskWriteResult<T> = std::result::Result<T, TaskWriteError>;

impl From<Error> for TaskWriteError {
    fn from(error: Error) -> Self {
        Self::Service(error)
    }
}
impl From<TaskTransformError> for TaskWriteError {
    fn from(error: TaskTransformError) -> Self {
        Self::Transform(error)
    }
}
impl fmt::Display for TaskWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Service(error) => error.fmt(formatter),
            Self::Transform(error) => formatter.write_str(&error.message),
        }
    }
}
impl std::error::Error for TaskWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Service(error) => Some(error),
            Self::Transform(error) => Some(error),
        }
    }
}
