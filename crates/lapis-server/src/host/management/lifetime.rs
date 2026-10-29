use super::Management;
use crate::host::{Result, error};
use lapis_app_protocol::ErrorCode;
use std::sync::{Arc, Condvar, Mutex};

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(super) struct Lifetime {
    state: Mutex<(bool, usize)>,
    drained: Condvar,
}
pub(super) struct Accepted(pub Arc<Management>);
impl Management {
    pub(super) fn accept(self: &Arc<Self>) -> Result<Accepted> {
        let mut state = self.lifetime.state.lock().map_err(|_| closed())?;
        if state.0 {
            return Err(closed());
        }
        state.1 += 1;
        Ok(Accepted(self.clone()))
    }
    pub fn pause_and_drain(&self) -> Result<()> {
        let mut state = self.lifetime.state.lock().map_err(|_| closed())?;
        state.0 = true;
        while state.1 != 0 {
            state = self.lifetime.drained.wait(state).map_err(|_| closed())?;
        }
        Ok(())
    }
    pub fn resume(&self) {
        if let Ok(mut state) = self.lifetime.state.lock() {
            state.0 = false;
        }
    }
}
impl Drop for Accepted {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.lifetime.state.lock() {
            state.1 -= 1;
            self.0.lifetime.drained.notify_all();
        }
    }
}
fn closed() -> lapis_app_protocol::ServiceError {
    error(ErrorCode::Closed, "Vault management is closing")
}
