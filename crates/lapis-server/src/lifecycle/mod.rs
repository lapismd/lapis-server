use crate::host::Host;
use std::{io, sync::Arc};

/// Register termination before serving. A blocked graceful leave keeps HTTP live
/// so the owner can explicitly recover retained editor claims, then retry.
pub struct Shutdown {
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
}
impl Shutdown {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            #[cfg(unix)]
            terminate: tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?,
        })
    }
    pub async fn wait(mut self, host: Arc<Host>) {
        loop {
            #[cfg(unix)]
            let signal = tokio::select! {
                result = tokio::signal::ctrl_c() => result,
                _ = self.terminate.recv() => Ok(()),
            };
            #[cfg(not(unix))]
            let signal = tokio::signal::ctrl_c().await;
            if signal.is_err() {
                eprintln!("Shutdown signal handler failed");
                std::future::pending::<()>().await;
            }
            let host = host.clone();
            match tokio::task::spawn_blocking(move || host.shutdown()).await {
                Ok(Ok(())) => return,
                _ => eprintln!("Shutdown blocked: resolve retained editor state before retrying"),
            }
        }
    }
}
pub mod maintenance;
pub mod server;
