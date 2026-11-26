use super::*;
use hickory_resolver::{
    Resolver,
    config::{ConnectionConfig, NameServerConfig, ResolverConfig},
    net::runtime::TokioRuntimeProvider,
};
use std::sync::{Arc, atomic::AtomicBool, mpsc};

#[test]
fn stalled_dns_is_cancelled_without_blocking_runtime_shutdown() {
    for cancel in [false, true] {
        let (done, received) = mpsc::channel();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async {
                // A real local resolver receives the query but never replies.
                let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
                let address = socket.local_addr().unwrap();
                let cancelled = Arc::new(AtomicBool::new(false));
                let observed = cancelled.clone();
                let (query_sent, query_received) = tokio::sync::oneshot::channel();
                let peer = tokio::spawn(async move {
                    socket.recv_from(&mut [0; 4096]).await.unwrap();
                    let _ = query_sent.send(());
                    if cancel {
                        observed.store(true, Ordering::Release);
                    }
                    std::future::pending::<()>().await;
                    drop(socket);
                });
                let mut connection = ConnectionConfig::udp();
                connection.port = address.port();
                let config = ResolverConfig::from_parts(
                    None,
                    vec![],
                    vec![NameServerConfig::new(address.ip(), true, vec![connection])],
                );
                let resolver =
                    Resolver::builder_with_config(config, TokioRuntimeProvider::default())
                        .build()
                        .unwrap();
                let result = network_wait(
                    resolver.lookup_ip("dns-stall.example.com."),
                    Instant::now() + Duration::from_millis(250),
                    || {
                        if cancelled.load(Ordering::Acquire) {
                            Err(Error::denied())
                        } else {
                            Ok(())
                        }
                    },
                )
                .await;
                assert!(result.is_err());
                query_received.await.unwrap();
                peer.abort();
            });
            drop(runtime);
            done.send(()).unwrap();
        });
        received
            .recv_timeout(Duration::from_secs(3))
            .expect("DNS cancellation must not leave blocking resolver tasks at shutdown");
    }
}
