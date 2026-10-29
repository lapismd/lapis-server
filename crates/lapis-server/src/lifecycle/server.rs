use super::{Shutdown, maintenance::Maintenance};
use crate::host::Host;
use std::{future::IntoFuture, io, sync::Arc, time::Duration};

/// The standalone binary exits its owned runtime after this transport drain.
/// The timeout starts only after native writers/jobs pass their leave gate.
pub async fn serve(
    listener: tokio::net::TcpListener,
    router: axum::Router,
    host: Arc<Host>,
    shutdown: Shutdown,
) -> io::Result<()> {
    let maintenance = Maintenance::start(host.clone());
    let (started, draining) = tokio::sync::oneshot::channel();
    let server = axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown.wait(host).await;
        maintenance.stop().await;
        let _ = started.send(());
    })
    .into_future();
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => result,
        signal = draining => {
            if signal.is_err() { return server.await; }
            match tokio::time::timeout(Duration::from_secs(10), &mut server).await {
                Ok(result) => result,
                Err(_) => {
                    eprintln!("HTTP drain deadline reached after native shutdown");
                    Ok(())
                }
            }
        }
    }
}
