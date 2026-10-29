use super::{Result, Work};
use crate::context::bootstrap::{BootstrapInput, PreparedBootstrap, prepare_bootstrap};
use lapis_app_protocol::ErrorCode;

impl Work {
    pub fn bootstrap_context(
        &self,
        input: &BootstrapInput,
        cancelled: impl Fn() -> bool,
    ) -> Result<PreparedBootstrap> {
        {
            let state = self.vault.lock()?;
            state.require_open()?;
            state.root.verify_physical_root()?;
            if !input.scope_dir.is_empty() {
                state.root.open_child(&input.scope_dir)?;
            }
        }
        let mut read = |path: &str| {
            let result = match self.read_text(path) {
                Ok(snapshot) => Ok(Some(snapshot.text)),
                Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
                Err(error) => Err(error),
            };
            self.vault.lock()?.root.verify_physical_root()?;
            result
        };
        let value = prepare_bootstrap(input, Some(&mut read), || {
            cancelled() || self.cancellation_requested()
        });
        let state = self.vault.lock()?;
        state.require_open()?;
        state.root.verify_physical_root()?;
        value
    }
}
