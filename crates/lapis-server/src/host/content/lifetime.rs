use super::{Access, Result, error};
use lapis_app_protocol::ErrorCode;
use std::sync::Arc;

pub(crate) struct Accepted(Arc<Access>);
impl Access {
    pub fn accept(self: &Arc<Self>) -> Result<Accepted> {
        let mut state = self.lifetime.lock().map_err(|_| closed())?;
        if state.0 {
            return Err(closed());
        }
        if state.1 >= 8 {
            return Err(error(ErrorCode::Busy, "Content controls are busy"));
        }
        state.1 += 1;
        Ok(Accepted(self.clone()))
    }
    pub fn closed(&self) -> bool {
        self.lifetime.lock().map_or(true, |state| state.0)
    }
    pub fn stop_and_drain(&self) -> Result<()> {
        let mut state = self.lifetime.lock().map_err(|_| closed())?;
        state.0 = true;
        while state.1 != 0 {
            state = self.drained.wait(state).map_err(|_| closed())?;
        }
        Ok(())
    }
}
impl Drop for Accepted {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.lifetime.lock() {
            state.1 -= 1;
            self.0.drained.notify_all();
        }
    }
}
fn closed() -> lapis_app_protocol::ServiceError {
    error(
        ErrorCode::Closed,
        "Content access is closing; restart the server to enable it",
    )
}
