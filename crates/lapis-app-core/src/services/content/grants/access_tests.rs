use super::*;
use crate::services::Runtime;
use std::{
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};

fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "synchronization deadline");
        thread::yield_now();
    }
}
fn read(vault_id: Uuid) -> ContentRequest {
    ContentRequest {
        api_version: 1,
        request_id: Uuid::new_v4(),
        command: ContentCommand::ReadNote {
            vault_id,
            path: "note.md".into(),
        },
    }
}

#[test]
fn disabling_fences_all_readers_before_drain_and_serializes_reenable() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("note.md"), "saved").unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, directory.path()).unwrap();
    let grants = ContentGrants::default();
    let issue = || {
        grants
            .issue_named_read_only(
                "Agent",
                vec![ContentVault {
                    name: "Personal".into(),
                    client: vault.attach().unwrap(),
                }],
                Duration::from_secs(60),
            )
            .unwrap()
    };
    let a = issue();
    let b = issue();
    let ae = grants.authenticate(a.token.expose_secret()).unwrap();
    let be = grants.authenticate(b.token.expose_secret()).unwrap();
    let native = vault.lock().unwrap();
    thread::scope(|scope| {
        let ar = scope.spawn(|| grants.execute(a.token.expose_secret(), read(id)));
        let br = scope.spawn(|| grants.execute(b.token.expose_secret(), read(id)));
        wait_for(|| ae.session.state.try_lock().is_err() && be.session.state.try_lock().is_err());
        let disabler = scope.spawn(|| grants.set_enabled(false));
        wait_for(|| {
            ae.session.revoked.load(Ordering::Acquire) && be.session.revoked.load(Ordering::Acquire)
        });
        let status = grants.status().unwrap();
        assert!(!status.enabled);
        assert!(status.grants.iter().all(|grant| !grant.active));
        assert!(grants.transition.try_lock().is_err());
        assert_eq!(
            grants
                .issue_read_only(vec![], Duration::from_secs(60))
                .unwrap_err()
                .code,
            ContentErrorCode::Unavailable
        );
        let enabler = scope.spawn(|| grants.set_enabled(true));
        assert!(!grants.status().unwrap().enabled);
        drop(native);
        for reader in [ar, br] {
            let ContentOutcome::Error { error } = reader.join().unwrap().outcome else {
                panic!("read must be fenced")
            };
            assert_eq!(error.code, ContentErrorCode::Unauthenticated);
        }
        disabler.join().unwrap().unwrap();
        enabler.join().unwrap().unwrap();
    });
    assert!(grants.status().unwrap().enabled);
    assert!(grants.status().unwrap().grants.is_empty());
    assert_eq!(vault.status().unwrap().clients, 0);
    for issued in [&a, &b] {
        assert!(
            grants
                .check_credential(issued.token.expose_secret())
                .is_err()
        );
    }
}

#[test]
fn admission_started_before_disable_or_retirement_cannot_publish_afterward() {
    for retirement in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let runtime = Runtime::default();
        let id = Uuid::new_v4();
        let vault = runtime.admit(id, directory.path()).unwrap();
        let grants = ContentGrants::default();
        let client = vault.attach().unwrap();
        let native = vault.lock().unwrap();
        thread::scope(|scope| {
            let issuer = scope.spawn(|| {
                grants.issue_named_read_only(
                    "Pending",
                    vec![ContentVault {
                        name: "Personal".into(),
                        client,
                    }],
                    Duration::from_secs(60),
                )
            });
            // Issuance has captured the current epoch and is waiting on the native lock.
            wait_for(|| Arc::strong_count(&grants.lock().unwrap().epoch) == 2);
            if retirement {
                assert_eq!(grants.revoke_vault(id).unwrap(), 0);
            } else {
                grants.set_enabled(false).unwrap();
                grants.set_enabled(true).unwrap();
            }
            drop(native);
            assert_eq!(
                issuer.join().unwrap().unwrap_err().code,
                ContentErrorCode::Unavailable
            );
        });
        assert!(grants.status().unwrap().grants.is_empty());
        assert_eq!(vault.status().unwrap().clients, 0);
    }
}

#[test]
fn expiry_status_uses_the_session_deadline_without_releasing_capacity() {
    let grants = ContentGrants::default();
    let issued = grants
        .issue_read_only(vec![], Duration::from_secs(60))
        .unwrap();
    let id = issued.grant_id;
    {
        let mut registry = grants.lock().unwrap();
        let entry = Arc::get_mut(registry.entries.get_mut(&id).unwrap()).unwrap();
        entry.session.expires = Instant::now();
    }
    let status = grants.status().unwrap();
    assert!(status.enabled);
    assert_eq!(status.grants.len(), 1);
    assert!(!status.grants[0].active);
    assert_eq!(status.grants[0].expires_in_seconds, 0);
    assert!(
        grants
            .check_credential(issued.token.expose_secret())
            .is_err()
    );
    assert_eq!(grants.prune_expired().unwrap(), 1);
    assert!(grants.status().unwrap().grants.is_empty());
}

#[test]
fn failed_drain_stays_fenced_and_cannot_be_reenabled() {
    let grants = ContentGrants::default();
    let issued = grants
        .issue_read_only(vec![], Duration::from_secs(60))
        .unwrap();
    let independent = grants
        .issue_read_only(vec![], Duration::from_secs(60))
        .unwrap();
    let entry = grants.authenticate(issued.token.expose_secret()).unwrap();
    assert!(
        std::panic::catch_unwind(|| {
            let _guard = entry.session.state.lock().unwrap();
            panic!("simulate a poisoned native session");
        })
        .is_err()
    );
    assert_eq!(
        grants.set_enabled(false).unwrap_err().code,
        ContentErrorCode::Unavailable
    );
    let status = grants.status().unwrap();
    assert!(!status.enabled);
    assert_eq!(status.grants.len(), 1);
    assert_eq!(status.grants[0].grant_id, issued.grant_id);
    assert!(!status.grants[0].active);
    assert!(grants.set_enabled(true).is_err());
    assert!(!grants.status().unwrap().enabled);
    assert!(
        grants
            .check_credential(issued.token.expose_secret())
            .is_err()
    );
    assert!(
        grants
            .check_credential(independent.token.expose_secret())
            .is_err()
    );
}

#[test]
fn vault_retirement_fences_all_matching_sessions_before_waiting_for_drain() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, directory.path()).unwrap();
    let grants = ContentGrants::default();
    let issue = || {
        grants
            .issue_named_read_only(
                "Agent",
                vec![ContentVault {
                    name: "Personal".into(),
                    client: vault.attach().unwrap(),
                }],
                Duration::from_secs(60),
            )
            .unwrap()
    };
    let a = issue();
    let b = issue();
    let independent = grants
        .issue_read_only(vec![], Duration::from_secs(60))
        .unwrap();
    let ae = grants.authenticate(a.token.expose_secret()).unwrap();
    let be = grants.authenticate(b.token.expose_secret()).unwrap();
    let held_a = ae.session.state.lock().unwrap();
    let held_b = be.session.state.lock().unwrap();
    thread::scope(|scope| {
        let retirement = scope.spawn(|| grants.revoke_vault(id));
        wait_for(|| {
            ae.session.revoked.load(Ordering::Acquire) && be.session.revoked.load(Ordering::Acquire)
        });
        grants
            .check_credential(independent.token.expose_secret())
            .unwrap();
        assert!(grants.status().unwrap().enabled);
        assert!(grants.check_credential(a.token.expose_secret()).is_err());
        assert!(grants.check_credential(b.token.expose_secret()).is_err());
        drop(held_a);
        drop(held_b);
        assert_eq!(retirement.join().unwrap().unwrap(), 2);
    });
    assert_eq!(vault.status().unwrap().clients, 0);
    assert_eq!(grants.status().unwrap().grants.len(), 1);
}
