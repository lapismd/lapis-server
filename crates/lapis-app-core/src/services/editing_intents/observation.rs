use super::{Client, Result, State};
use tokio::sync::watch;

impl State {
    pub(in crate::services) fn invalidate_editing_intents(&self) {
        self.editing_intent_changes.send_modify(|revision| {
            *revision = revision.wrapping_add(1);
        });
    }
}

impl Client {
    /// Subscribe under the same lock as mutations; one latest value bounds lag.
    pub fn subscribe_to_editing_intents(&self) -> Result<watch::Receiver<u64>> {
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        state.root.verify_physical_root()?;
        Ok(state.editing_intent_changes.subscribe())
    }

    pub fn validate_editing_intent_observation(&self) -> Result<()> {
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        state.root.verify_physical_root()?;
        Ok(())
    }
}
