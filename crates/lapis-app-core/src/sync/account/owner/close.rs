use super::{
    AccountCommand, AccountObservation, AccountReply, Closing, State, busy, run, unavailable,
};
use std::{
    io,
    sync::{Arc, atomic::Ordering},
};
use tokio::{runtime::Handle, sync::OwnedSemaphorePermit, task::JoinHandle};

/// Native lifecycle admission, not a renderer-owned account command.
pub struct AccountClosePreparation(JoinHandle<io::Result<PreparedAccountClose>>);
impl AccountClosePreparation {
    pub async fn wait(self) -> io::Result<PreparedAccountClose> {
        self.0.await.map_err(|_| unavailable())?
    }
}

/// Unforgeable original-scope guard. No Clone, Debug or presentation serialization.
/// Dropping it releases only the admission fence; the original session survives.
pub struct PreparedAccountClose {
    state: Arc<State>,
    permit: OwnedSemaphorePermit,
    _closing: Closing,
}
impl PreparedAccountClose {
    /// Blocking native commit after all other leave guards approve.
    /// The host calls this off UI/async executors; a failed close retains state.
    pub fn commit(self) -> io::Result<()> {
        let Self {
            state,
            permit,
            _closing,
        } = self;
        match run(state, AccountCommand::Close, permit)? {
            AccountReply::Closed => Ok(()),
            _ => Err(unavailable()),
        }
    }
}

pub(super) fn prepare(state: Arc<State>, runtime: Handle) -> io::Result<AccountClosePreparation> {
    state
        .closing
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| busy())?;
    let closing = Closing(state.clone());
    Ok(AccountClosePreparation(runtime.spawn(async move {
        // Admission waits asynchronously, never occupying a blocking worker.
        let permit = state
            .single_flight
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| unavailable())?;
        tokio::task::spawn_blocking(move || {
            let slot = state.session.lock().map_err(|_| unavailable())?;
            if let Some(session) = slot.as_ref()
                && let Err(error) = session.prepare_close()
            {
                state
                    .observation
                    .send_replace(AccountObservation::RecoveryRequired {
                        custody_pending: session.custody_pending(),
                    });
                return Err(error);
            }
            drop(slot);
            Ok(PreparedAccountClose {
                state,
                permit,
                _closing: closing,
            })
        })
        .await
        .map_err(|_| unavailable())?
    })))
}
