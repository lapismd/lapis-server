use super::failure;
use crate::services::{Error, Result, VaultService};
use lapis_app_protocol::VaultPhase;

impl VaultService {
    /// Called on a native blocking worker, before database admission is fenced.
    /// The original coordinator and watch survive a failed final capture.
    pub(in crate::services) fn finish_backup(&self) -> Result<()> {
        {
            let mut state = self.lock()?;
            if state.phase == VaultPhase::Closed {
                return Ok(());
            }
            if state.editors_blocked(None) {
                return Err(Error::busy());
            }
            state.phase = VaultPhase::Closing;
            state.backup_pending = true;
            while state.work != 0 {
                if self.turns_failed() {
                    state.backup_pending = false;
                    self.changed.notify_all();
                    return Err(Error::busy());
                }
                state = self.changed.wait(state).map_err(|_| Error::closed())?;
            }
        }
        let result = (|| {
            let owner = self.backup.lock().map_err(|_| failure())?.clone();
            if let Some(owner) = owner {
                let mut engine = owner.engine.blocking_lock();
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| failure())?
                    .block_on(engine.try_close())
                    .map_err(|_| failure())?;
                owner.watch.lock().map_err(|_| failure())?.take();
            }
            Ok(())
        })();
        let mut state = self.lock()?;
        state.backup_pending = false;
        if result.is_err() {
            state.phase = VaultPhase::Open;
        }
        self.changed.notify_all();
        result
    }
}
