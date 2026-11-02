use super::initialization::{Shutdown, ShutdownStage};
use super::{Owner, diagnostics, failure};
use crate::services::{Client, Error, Result, VaultService};
use jj_vault::Backup;
use std::{future::Future, pin::Pin, sync::Arc};

impl VaultService {
    fn selected_backup(&self) -> Result<Arc<Owner>> {
        let storage = self.backup_storage.as_ref().ok_or_else(|| {
            Error::new(
                lapis_app_protocol::ErrorCode::Unsupported,
                "Automatic backup is not selected",
            )
        })?;
        storage.validate_vault(&self.lock()?.root)?;
        self.backup
            .lock()
            .map_err(|_| failure())?
            .clone()
            .ok_or_else(|| {
                Error::new(
                    lapis_app_protocol::ErrorCode::Unsupported,
                    "Automatic backup has not started",
                )
            })
    }
}
impl Client {
    /// Native hosts borrow commands, never coordinator ownership. The service
    /// task owns accepted work and its engine guard independently of the reply.
    pub async fn with_backup<T: Send + 'static>(
        &self,
        action: impl for<'a> FnOnce(
            &'a Backup,
        ) -> Pin<
            Box<dyn Future<Output = std::result::Result<T, String>> + Send + 'a>,
        > + Send
        + 'static,
    ) -> Result<T> {
        self.vault.lock()?.require_client(self.id)?;
        let work = self.vault.begin_work()?;
        let vault = self.vault.clone();
        let (reply, receiver) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let result = async {
                let owner = tokio::task::spawn_blocking(move || vault.selected_backup())
                    .await
                    .map_err(|_| failure())??;
                let engine = owner.engine.lock().await;
                action(&engine).await.map_err(diagnostics::command)
            }
            .await;
            drop(work);
            let _ = reply.send(result);
        });
        receiver.await.map_err(|_| failure())?
    }

    /// Explicit disable retains failed workers for retry, and removes only a
    /// successfully drained coordinator. A detached observer never calls this.
    pub async fn stop_backup(&self) -> Result<()> {
        let diagnostics = Shutdown::new();
        diagnostics.service(
            ShutdownStage::ClientAdmission,
            (|| self.vault.lock()?.require_client(self.id))(),
        )?;
        let work = diagnostics.service(ShutdownStage::WorkReservation, self.vault.begin_work())?;
        let vault = self.vault.clone();
        let ticket =
            diagnostics.service(ShutdownStage::QueueReservation, vault.backup.reserve())?;
        let worker_diagnostics = diagnostics.clone();
        let result = ticket
            .run(work, move || {
                let storage = worker_diagnostics.service(
                    ShutdownStage::SelectStorage,
                    vault.backup_storage.as_ref().ok_or_else(|| {
                        Error::new(
                            lapis_app_protocol::ErrorCode::Unsupported,
                            "Automatic backup is not selected",
                        )
                    }),
                )?;
                worker_diagnostics.service(
                    ShutdownStage::ValidateStorage,
                    (|| storage.validate_vault(&vault.lock()?.root))(),
                )?;
                let mut slot = worker_diagnostics.service(
                    ShutdownStage::LockOwner,
                    vault.backup.lock().map_err(|_| failure()),
                )?;
                stop_observed(&mut slot, &worker_diagnostics)
            })
            .await;
        diagnostics.service(ShutdownStage::QueueCompletion, result)
    }
}

pub(super) fn stop(slot: &mut Option<Arc<Owner>>) -> Result<()> {
    stop_inner(slot, None)
}

fn stop_observed(slot: &mut Option<Arc<Owner>>, diagnostics: &Shutdown) -> Result<()> {
    stop_inner(slot, Some(diagnostics))
}

fn stop_inner(slot: &mut Option<Arc<Owner>>, diagnostics: Option<&Shutdown>) -> Result<()> {
    let Some(owner) = slot.clone() else {
        return Ok(());
    };
    let mut engine = owner.engine.blocking_lock();
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| failure())?
        .block_on(engine.try_close());
    match diagnostics {
        Some(diagnostics) => diagnostics.native(ShutdownStage::FinalCapture, result)?,
        None => result.map_err(|_| failure())?,
    }
    let mut watcher = match diagnostics {
        Some(diagnostics) => diagnostics.service(
            ShutdownStage::ReleaseWatcher,
            owner.watch.lock().map_err(|_| failure()),
        )?,
        None => owner.watch.lock().map_err(|_| failure())?,
    };
    watcher.take();
    *slot = None;
    Ok(())
}
