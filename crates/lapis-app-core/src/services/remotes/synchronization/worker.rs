use super::*;
use std::{sync::Weak, time::Duration};

pub(super) fn run(vault: Weak<VaultService>, control: Arc<Control>) {
    let mut seen = 0;
    let mut delay = Duration::from_secs(5);
    loop {
        let Ok(requested) = control.requested.lock() else {
            return;
        };
        let Ok((requested, _)) = control.changed.wait_timeout_while(requested, delay, |v| {
            *v == seen && !control.stopped.load(Ordering::Acquire)
        }) else {
            return;
        };
        seen = *requested;
        drop(requested);
        if control.stopped.load(Ordering::Acquire) {
            return;
        }
        let Some(vault) = vault.upgrade() else {
            return;
        };
        if vault
            .status()
            .is_ok_and(|s| s.phase == lapis_app_protocol::VaultPhase::Closed)
        {
            return;
        }
        match ready(&vault, &control) {
            Ok(true) => {}
            Ok(false) => {
                delay = Duration::from_secs(60);
                continue;
            }
            Err(_) => {
                let _ = control.update(|s| {
                    if !s.busy {
                        s.phase = RepositorySyncPhase::Attention;
                    }
                });
                delay = Duration::from_secs(60);
                continue;
            }
        }
        // Closing refuses new work; a failed close restores Open and this same
        // scheduler resumes on its next turn. No permanent lease prevents close.
        let Ok(work) = vault.begin_work() else {
            delay = Duration::from_secs(1);
            continue;
        };
        let Ok(turn) = vault.backup.reserve() else {
            return;
        };
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        let result = runtime.block_on(turn.run(work, move || cycle::execute(&vault, None)));
        if result.is_err() {
            let _ = control.update(|s| {
                s.phase = RepositorySyncPhase::Attention;
                s.busy = false;
            });
        }
        delay = Duration::from_secs(60);
    }
}
fn ready(vault: &Arc<VaultService>, control: &Control) -> Result<bool> {
    if control.status.lock().map_err(|_| Error::busy())?.busy {
        return Ok(false);
    }
    let storage = vault.remotes_storage.as_ref().ok_or_else(unsupported)?;
    let selection = storage.selection(vault)?;
    let state = selection.read()?;
    vault.synchronization.selection(&state);
    if state.backup.is_none() {
        return Ok(false);
    }
    let saved = journal::Journal::open(&selection, &state, &backup::source(vault)?)?;
    control.update(|s| {
        if !s.busy {
            observation::populate(s, &saved, &state);
        }
    })?;
    Ok(state.automatic_sync
        && saved.pending.is_none()
        && !saved.proposal.is_some_and(|p| !p.conflicts.is_empty()))
}
