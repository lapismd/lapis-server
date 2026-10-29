use crate::host::Host;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub struct Maintenance(Option<tokio::task::JoinHandle<()>>);
impl Maintenance {
    pub fn start(host: Arc<Host>) -> Self {
        Self(Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let host = host.clone();
                if !matches!(
                    tokio::task::spawn_blocking(move || host.expire_idle(Instant::now())).await,
                    Ok(Ok(_))
                ) {
                    eprintln!("Attachment maintenance stopped: host is unavailable");
                    return;
                }
            }
        })))
    }
    pub async fn stop(mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
            let _ = task.await;
        }
    }
}
impl Drop for Maintenance {
    fn drop(&mut self) {
        if let Some(task) = &self.0 {
            task.abort();
        }
    }
}
