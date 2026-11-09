use super::*;
use std::{sync::atomic::Ordering, time::Instant};

impl ContentGrants {
    /// Production hosts start disabled, then restore owner-approved settings.
    /// This owns native access only; hosts own listeners and persistent consent.
    pub fn disabled() -> Self {
        Self {
            registry: Mutex::new(Registry {
                paused: true,
                ..Default::default()
            }),
            transition: Mutex::new(()),
        }
    }

    pub(super) fn transition(&self) -> Result<MutexGuard<'_, ()>> {
        self.transition.lock().map_err(|_| {
            failure(
                ContentErrorCode::Unavailable,
                "Content controls unavailable",
            )
        })
    }

    /// Owner-only, blocking. Fence all authority before waiting for readers.
    /// Re-enable waits for disable/drain and never revives retired credentials.
    pub fn set_enabled(&self, enabled: bool) -> Result<()> {
        let _transition = self.transition()?;
        if !enabled {
            return self.suspend(false);
        }
        {
            let registry = self.lock()?;
            if registry.closed {
                return Err(failure(
                    ContentErrorCode::Unavailable,
                    "Content grants shut down",
                ));
            }
            if !registry.paused {
                return Ok(());
            }
        }
        // Retry any failed drain before reopening admission.
        self.prune_expired()?;
        self.lock()?.paused = false;
        Ok(())
    }

    /// Cheap host transport gate; does not authenticate a content operation.
    pub fn is_enabled(&self) -> Result<bool> {
        let registry = self.lock()?;
        Ok(!registry.closed && !registry.paused)
    }

    /// Owner-only snapshot; does not clean up expired entries or expose secrets.
    /// No native IO/lock wait occurs while holding the registry lock.
    pub fn status(&self) -> Result<ContentAccessStatus> {
        let registry = self.lock()?;
        let enabled = !registry.closed && !registry.paused;
        let now = Instant::now();
        let grants = registry
            .entries
            .iter()
            .map(|(id, entry)| {
                let remaining = entry.session.expires.saturating_duration_since(now);
                let seconds = remaining.as_secs() + u64::from(remaining.subsec_nanos() != 0);
                ContentGrantSummary {
                    grant_id: *id,
                    label: entry.label.clone(),
                    vault_ids: entry.vault_ids.clone(),
                    scope: ContentScope::Read,
                    active: enabled && entry.session.require_live().is_ok(),
                    expires_in_seconds: seconds.min(86_400) as u32,
                }
            })
            .collect();
        Ok(ContentAccessStatus { enabled, grants })
    }

    /// Retire entire grants containing a vault, fencing all before any drain.
    /// The host must also fence new native admission while retiring the vault.
    /// This invalidates already-started issuance, including unrelated scopes.
    pub fn revoke_vault(&self, vault_id: Uuid) -> Result<usize> {
        let retired = {
            let mut registry = self.lock()?;
            registry.epoch = Arc::new(());
            registry
                .entries
                .iter()
                .filter(|(_, entry)| entry.vault_ids.contains(&vault_id))
                .map(|(id, entry)| {
                    entry.session.revoked.store(true, Ordering::Release);
                    (*id, Arc::clone(entry))
                })
                .collect()
        };
        self.drain(retired)
    }
}
