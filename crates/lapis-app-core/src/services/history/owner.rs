use super::{HistoryMethod, frames, policy::Policy};
use crate::services::{VaultService, Work};
use jj_history_service::{Error, History, Result};
use lapis_app_protocol::repository_sources::RepositorySources;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, Weak, mpsc},
    thread,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};
use uuid::Uuid;

enum Command {
    Sources {
        client: Uuid,
        reply: oneshot::Sender<Result<RepositorySources>>,
        _work: Work,
        _permit: OwnedSemaphorePermit,
    },
    Call {
        client: Uuid,
        method: HistoryMethod,
        frame: String,
        reply: oneshot::Sender<Result<String>>,
        _work: Work,
        _permit: OwnedSemaphorePermit,
    },
    Revoke(Uuid, mpsc::SyncSender<Result<()>>),
    Shutdown,
}
/// One vault's retained executor. HTTP waiters never own its preparation tasks.
pub(in crate::services) struct Owner {
    sender: mpsc::Sender<Command>,
    queue: Arc<Semaphore>,
    thread: Mutex<Option<thread::JoinHandle<Result<()>>>>,
}
impl Owner {
    pub fn start(vault: Weak<VaultService>, cache: PathBuf) -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let (ready, started) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("lapis-history-reader".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .thread_name("lapis-history-work")
                    .enable_all()
                    .build()
                    .map_err(|_| Error::Unavailable)?;
                let policy = Policy::new(vault);
                let history = History::new(policy.clone(), cache, Default::default())?;
                ready.send(()).map_err(|_| Error::Unavailable)?;
                for command in receiver {
                    match command {
                        Command::Sources {
                            client,
                            reply,
                            _work,
                            _permit,
                        } => {
                            let _ = reply.send(policy.open_original(client));
                        }
                        Command::Call {
                            client,
                            method,
                            frame,
                            reply,
                            _work,
                            _permit,
                        } => {
                            let result = runtime
                                .block_on(frames::call(&history, &policy, client, method, &frame));
                            let _ = reply.send(result);
                        }
                        Command::Revoke(client, reply) => {
                            // The Core client has already been fenced. No queued
                            // call can create another scope after this cleanup.
                            let result = runtime
                                .block_on(history.revoke(&client.to_string()))
                                .and_then(|_| policy.revoke(client));
                            let _ = reply.send(result);
                        }
                        Command::Shutdown => break,
                    }
                }
                runtime.block_on(history.shutdown())
            })
            .map_err(|_| Error::Unavailable)?;
        if started.recv().is_err() {
            let _ = thread.join();
            return Err(Error::Unavailable);
        }
        Ok(Self {
            sender,
            queue: Arc::new(Semaphore::new(32)),
            thread: Mutex::new(Some(thread)),
        })
    }
    pub async fn call(
        &self,
        client: Uuid,
        method: HistoryMethod,
        frame: String,
        work: Work,
    ) -> Result<String> {
        let permit = self
            .queue
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy)?;
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Command::Call {
                client,
                method,
                frame,
                reply,
                _work: work,
                _permit: permit,
            })
            .map_err(|_| Error::Unavailable)?;
        response.await.map_err(|_| Error::Unavailable)?
    }
    pub async fn sources(&self, client: Uuid, work: Work) -> Result<RepositorySources> {
        let permit = self
            .queue
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy)?;
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Command::Sources {
                client,
                reply,
                _work: work,
                _permit: permit,
            })
            .map_err(|_| Error::Unavailable)?;
        response.await.map_err(|_| Error::Unavailable)?
    }
    pub fn revoke(&self, client: Uuid) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.sender
            .send(Command::Revoke(client, reply))
            .map_err(|_| Error::Unavailable)?;
        response.recv().map_err(|_| Error::Unavailable)?
    }
    pub fn finish(&self) -> Result<()> {
        let mut slot = self.thread.lock().map_err(|_| Error::Unavailable)?;
        if let Some(thread) = slot.take() {
            let _ = self.sender.send(Command::Shutdown);
            return thread.join().map_err(|_| Error::Unavailable)?;
        }
        Ok(())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Shutdown);
        if let Some(thread) = self
            .thread
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            // An abandoned background Access may release the last vault Arc
            // on this executor. Join from a reaper instead of its own worker.
            let _ = thread::Builder::new()
                .name("lapis-history-drain".into())
                .spawn(move || {
                    let _ = thread.join();
                });
        }
    }
}
