use super::*;

/// Installation-owned executor. It shuts down on its own thread even when the
/// native host drops from asynchronous code.
pub(super) struct Executor {
    pub handle: tokio::runtime::Handle,
    shutdown: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}
impl Executor {
    pub fn new() -> Result<Arc<Self>> {
        let (ready, receive) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("lapis-job-executor".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(_) => {
                        let _ = ready.send(Err(invalid("Job executor unavailable")));
                        return;
                    }
                };
                let (shutdown, stop) = tokio::sync::oneshot::channel();
                if ready
                    .send(Ok(Arc::new(Self {
                        handle: runtime.handle().clone(),
                        shutdown: Mutex::new(Some(shutdown)),
                    })))
                    .is_err()
                {
                    return;
                }
                runtime.block_on(async {
                    let _ = stop.await;
                });
            })?;
        receive
            .recv()
            .map_err(|_| invalid("Job executor did not start"))?
    }
}
impl Drop for Executor {
    fn drop(&mut self) {
        if let Some(sender) = self
            .shutdown
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            let _ = sender.send(());
        }
    }
}
