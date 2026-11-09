use super::*;
use crate::services::Runtime;
use std::{fs, sync::atomic::Ordering, thread, time::Instant};

fn request(command: ContentCommand) -> ContentRequest {
    ContentRequest {
        api_version: 1,
        request_id: Uuid::new_v4(),
        command,
    }
}
fn capabilities() -> ContentRequest {
    request(ContentCommand::Capabilities {})
}
fn read(id: Uuid) -> ContentRequest {
    request(ContentCommand::ReadNote {
        vault_id: id,
        path: "note.md".into(),
    })
}
fn error(reply: ContentReply) -> ContentError {
    let ContentOutcome::Error { error } = reply.outcome else {
        panic!("expected refusal")
    };
    assert_eq!(error.effect, ContentEffectStatus::NotStarted);
    error
}
fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "synchronization deadline");
        thread::yield_now();
    }
}

#[test]
fn revocation_keeps_its_capacity_until_drain_without_blocking_other_grants() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("note.md"), "saved").unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, directory.path()).unwrap();
    let grants = ContentGrants::default();
    let issued = grants
        .issue_read_only(
            vec![ContentVault {
                name: "test".into(),
                client: vault.attach().unwrap(),
            }],
            Duration::from_secs(60),
        )
        .unwrap();
    let independent = grants
        .issue_read_only(vec![], Duration::from_secs(60))
        .unwrap();
    for _ in 2..MAX_CONTENT_GRANTS {
        grants
            .issue_read_only(vec![], Duration::from_secs(60))
            .unwrap();
    }
    let entry = grants.authenticate(issued.token.expose_secret()).unwrap();
    let native = vault.lock().unwrap();
    thread::scope(|scope| {
        let reader = scope.spawn(|| grants.execute(issued.token.expose_secret(), read(id)));
        wait_for(|| entry.session.state.try_lock().is_err());
        let revoker = scope.spawn(|| grants.revoke(issued.grant_id));
        wait_for(|| entry.session.revoked.load(Ordering::Acquire));
        assert!(matches!(
            grants
                .execute(independent.token.expose_secret(), capabilities())
                .outcome,
            ContentOutcome::Ok { .. }
        ));
        assert_eq!(
            grants
                .issue_read_only(vec![], Duration::from_secs(60))
                .unwrap_err()
                .code,
            ContentErrorCode::Busy
        );
        let refused = error(grants.execute(issued.token.expose_secret(), capabilities()));
        assert_eq!(refused.code, ContentErrorCode::Unauthenticated);
        assert_eq!(
            refused.message,
            error(grants.execute("bad", capabilities())).message
        );
        drop(native);
        assert_eq!(
            error(reader.join().unwrap()).code,
            ContentErrorCode::Unauthenticated
        );
        assert!(revoker.join().unwrap().unwrap());
    });
    assert_eq!(vault.status().unwrap().clients, 0);
    grants
        .issue_read_only(vec![], Duration::from_secs(60))
        .unwrap();
}

#[test]
fn expiry_refuses_before_cleanup_then_sweep_releases_attachments_and_capacity() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), directory.path()).unwrap();
    let grants = ContentGrants::default();
    let issued = grants
        .issue_read_only(
            vec![ContentVault {
                name: "test".into(),
                client: vault.attach().unwrap(),
            }],
            Duration::from_secs(60),
        )
        .unwrap();
    for _ in 1..MAX_CONTENT_GRANTS {
        grants
            .issue_read_only(vec![], Duration::from_secs(60))
            .unwrap();
    }
    // Set deadlines directly in this private test, without sleeping or production clock hooks.
    for entry in grants.lock().unwrap().entries.values_mut() {
        Arc::get_mut(entry).unwrap().session.expires = Instant::now();
    }
    let refused = error(grants.execute(issued.token.expose_secret(), capabilities()));
    assert_eq!(refused.code, ContentErrorCode::Unauthenticated);
    assert_eq!(
        refused.message,
        error(grants.execute("bad", capabilities())).message
    );
    assert_eq!(vault.status().unwrap().clients, 1);
    assert_eq!(
        grants
            .issue_read_only(vec![], Duration::from_secs(60))
            .unwrap_err()
            .code,
        ContentErrorCode::Busy
    );
    assert_eq!(grants.prune_expired().unwrap(), MAX_CONTENT_GRANTS);
    assert_eq!(vault.status().unwrap().clients, 0);
    assert_eq!(grants.prune_expired().unwrap(), 0);
    grants
        .issue_read_only(vec![], Duration::from_secs(60))
        .unwrap();
}

#[test]
fn shutdown_fences_every_native_waiter_before_draining_any_grant() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let aid = Uuid::new_v4();
    let bid = Uuid::new_v4();
    let av = runtime.admit(aid, a.path()).unwrap();
    let bv = runtime.admit(bid, b.path()).unwrap();
    let grants = ContentGrants::default();
    let ag = grants
        .issue_read_only(
            vec![ContentVault {
                name: "A".into(),
                client: av.attach().unwrap(),
            }],
            Duration::from_secs(60),
        )
        .unwrap();
    let bg = grants
        .issue_read_only(
            vec![ContentVault {
                name: "B".into(),
                client: bv.attach().unwrap(),
            }],
            Duration::from_secs(60),
        )
        .unwrap();
    let ae = grants.authenticate(ag.token.expose_secret()).unwrap();
    let be = grants.authenticate(bg.token.expose_secret()).unwrap();
    let an = av.lock().unwrap();
    let bn = bv.lock().unwrap();
    thread::scope(|scope| {
        let ar = scope.spawn(|| grants.execute(ag.token.expose_secret(), read(aid)));
        let br = scope.spawn(|| grants.execute(bg.token.expose_secret(), read(bid)));
        wait_for(|| ae.session.state.try_lock().is_err() && be.session.state.try_lock().is_err());
        let closer = scope.spawn(|| grants.shutdown());
        wait_for(|| {
            ae.session.revoked.load(Ordering::Acquire) && be.session.revoked.load(Ordering::Acquire)
        });
        assert_eq!(
            grants
                .issue_read_only(vec![], Duration::from_secs(60))
                .unwrap_err()
                .code,
            ContentErrorCode::Unavailable
        );
        drop(an);
        drop(bn);
        assert_eq!(
            error(ar.join().unwrap()).code,
            ContentErrorCode::Unauthenticated
        );
        assert_eq!(
            error(br.join().unwrap()).code,
            ContentErrorCode::Unauthenticated
        );
        closer.join().unwrap().unwrap();
    });
    assert_eq!(av.status().unwrap().clients, 0);
    assert_eq!(bv.status().unwrap().clients, 0);
    assert!(grants.lock().unwrap().entries.is_empty());
}
