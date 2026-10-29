use super::{Client, Data, Error, Owner, Result, failure};
use lapis_app_protocol::ErrorCode;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

impl Owner {
    pub(super) fn require_enabled(&self, data: &Data) -> Result<()> {
        if !data.enabled {
            return Err(Error::new(
                ErrorCode::Unsupported,
                "Native turns are disabled",
            ));
        }
        if self.failed.load(Ordering::Acquire) {
            return Err(Error::busy());
        }
        Ok(())
    }
}
impl Client {
    /// Native feature intent. The synchronous operation owns its fence/drain
    /// even when a host loses the blocking worker's response handle.
    pub fn set_turns_enabled(&self, enabled: bool) -> Result<bool> {
        let (owner, _work) = self.turn_owner()?;
        let _transition = owner.transitions.lock().map_err(|_| failure())?;
        let mut data = owner.data()?;
        if enabled {
            if owner.failed.load(Ordering::Acquire) {
                return Err(failure());
            }
            if !data.enabled && !data.jobs.is_empty() {
                return Err(Error::busy());
            }
            data.enabled = true;
            return Ok(true);
        }
        data.queues.pause("native_turns_disabled")?;
        data.enabled = false;
        for job in data.jobs.values() {
            job.cancel.send_replace(true);
        }
        let deadline = Instant::now() + Duration::from_secs(35);
        loop {
            if owner.failed.load(Ordering::Acquire) {
                return Err(failure());
            }
            if data.jobs.is_empty() {
                return Ok(false);
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(failure)?;
            let (next, _) = owner
                .changed
                .wait_timeout(data, remaining.min(Duration::from_millis(100)))
                .map_err(|_| failure())?;
            data = next;
        }
    }
}
