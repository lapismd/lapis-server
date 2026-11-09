use super::{ContentGrants, Entry, Result};
use std::sync::{Arc, atomic::Ordering};
use uuid::Uuid;

impl ContentGrants {
    /// Owner-only: fence first, drain without the registry lock, then free capacity.
    /// A missing grant is an idempotent no-op.
    pub fn revoke(&self, grant_id: Uuid) -> Result<bool> {
        let entry = {
            let registry = self.lock()?;
            let Some(entry) = registry.entries.get(&grant_id) else {
                return Ok(false);
            };
            entry.session.revoked.store(true, Ordering::Release);
            Arc::clone(entry)
        };
        self.drain(vec![(grant_id, entry)]).map(|count| count > 0)
    }

    /// Host-scheduled cleanup: expiry refuses access independently of this sweep.
    /// Failed drains remain fenced and continue occupying their capacity slot.
    pub fn prune_expired(&self) -> Result<usize> {
        let retired = {
            let registry = self.lock()?;
            registry
                .entries
                .iter()
                .filter(|(_, entry)| entry.session.require_live().is_err())
                .map(|(id, entry)| {
                    entry.session.revoked.store(true, Ordering::Release);
                    (*id, Arc::clone(entry))
                })
                .collect()
        };
        self.drain(retired)
    }

    /// Permanently stop issuance, fence every grant, then drain all accepted reads.
    pub fn shutdown(&self) -> Result<()> {
        let _transition = self.transition()?;
        self.suspend(true)
    }

    pub(super) fn suspend(&self, permanently: bool) -> Result<()> {
        let retired = {
            let mut registry = self.lock()?;
            registry.closed |= permanently;
            registry.paused = true;
            registry.epoch = Arc::new(());
            registry
                .entries
                .iter()
                .map(|(id, entry)| {
                    entry.session.revoked.store(true, Ordering::Release);
                    (*id, Arc::clone(entry))
                })
                .collect()
        };
        self.drain(retired).map(|_| ())
    }

    pub(super) fn drain(&self, retired: Vec<(Uuid, Arc<Entry>)>) -> Result<usize> {
        let mut count = 0;
        let mut failure = None;
        for (id, entry) in retired {
            match entry.session.revoke() {
                Ok(()) => {
                    count += usize::from(self.lock()?.entries.remove(&id).is_some());
                }
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(count),
        }
    }
}
