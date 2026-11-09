use crate::response::Failure;
use lapis_app_protocol::content::ContentErrorCode;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub(crate) async fn run<T: Send + 'static>(
    jobs: Arc<Semaphore>,
    action: impl FnOnce() -> Result<T, Failure> + Send + 'static,
) -> Result<T, Failure> {
    let permit = jobs
        .try_acquire_owned()
        .map_err(|_| Failure::new(ContentErrorCode::Busy, "Content native workers are busy"))?;
    tokio::task::spawn_blocking(move || {
        // Retain capacity even if the async caller is cancelled/disconnected.
        let _permit = permit;
        action()
    })
    .await
    .map_err(|_| Failure::new(ContentErrorCode::Unavailable, "Content worker unavailable"))?
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
