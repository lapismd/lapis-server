mod credentials;
#[path = "sync_account_session/support.rs"]
mod support;
use lapis_app_core::sync::account::{DiscoveryCredentialStatus, session::AccountPhase};
use std::sync::atomic::Ordering;
use support::{Device, Server, now};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn protected_native_account_restores_in_cold_process_and_signs_out() {
    const COLD: &str = "LAPIS_SYNC_ACCOUNT_SESSION_TEST_SCOPE";
    if let Some(path) = std::env::var_os(COLD) {
        use lapis_app_core::sync::account::owner::{AccountCommand, AccountOwner, AccountReply};
        let (owner, client, managed) = tokio::task::spawn_blocking(move || {
            let scope: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            let root = std::path::PathBuf::from(scope["root"].as_str().unwrap());
            std::env::set_current_dir(std::env::temp_dir()).unwrap();
            let id = |key| serde_json::from_value(scope[key].clone()).unwrap();
            let session = support::open(
                &root,
                id("profile"),
                id("vault"),
                scope["origin"].as_str().unwrap(),
                std::sync::Arc::new(support::FileCustody::new(root.join("fake-custody.json"))),
            );
            (AccountOwner::new(session), id("client"), id("managed"))
        })
        .await
        .unwrap();
        let AccountReply::Snapshot(snapshot) = owner
            .submit(AccountCommand::Snapshot)
            .unwrap()
            .wait()
            .await
            .unwrap()
        else {
            panic!("snapshot")
        };
        assert_eq!(snapshot.phase, AccountPhase::Connected);
        assert_eq!(snapshot.client_id, client);
        let AccountReply::Vaults(vaults) = owner
            .submit(AccountCommand::Discover { now: now() })
            .unwrap()
            .wait()
            .await
            .unwrap()
        else {
            panic!("discovery")
        };
        assert_eq!(vaults.len(), 1);
        assert_eq!(vaults[0].vault_id, managed);
        owner
            .submit(AccountCommand::Close)
            .unwrap()
            .wait()
            .await
            .unwrap();
        return;
    }
    let mut server = Server::start().await;
    let origin = server.origin.clone();
    let (device, mut session, waiting) = tokio::task::spawn_blocking(move || {
        let device = Device::new();
        let mut session = device.session(&origin);
        let waiting = session.begin("Independent Desktop", now()).unwrap();
        assert_eq!(waiting.phase, AccountPhase::AwaitingApproval);
        assert!(session.redeem(now()).is_err());
        session.close().unwrap();
        let session = device.session(&origin);
        assert_eq!(session.snapshot().unwrap(), waiting);
        (device, session, waiting)
    })
    .await
    .unwrap();
    server.approve(waiting.connection_id.unwrap()).await;
    let managed = server.managed;
    let origin = server.origin.clone();
    let (device, descriptor) = tokio::task::spawn_blocking(move || {
        let connected = session.redeem(now()).unwrap();
        assert_eq!(connected.phase, AccountPhase::Connected);
        assert!(connected.approval_url.is_none());
        let public = serde_json::to_value(&connected).unwrap();
        let keys: std::collections::BTreeSet<_> = public
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "client_id",
                "origin",
                "phase",
                "connection_id",
                "approval_url",
                "expires_at",
                "credential_expires_at",
                "abandon_after"
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(session.discover(now()).unwrap()[0].vault_id, managed);
        session.close().unwrap();
        let descriptor = device.descriptor(&origin, connected.client_id, managed);
        (device, descriptor)
    })
    .await
    .unwrap();
    tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "protected_native_account_restores_in_cold_process_and_signs_out",
                "--nocapture",
            ])
            .env(COLD, descriptor)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "cold native account process failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    })
    .await
    .unwrap();
    let origin = server.origin.clone();
    tokio::task::spawn_blocking(move || {
        let mut session = device.session(&origin);
        let client = session.snapshot().unwrap().client_id;
        assert_eq!(
            session.disconnect(now()).unwrap().phase,
            AccountPhase::Disconnected
        );
        session.close().unwrap();
        let mut restored = device.session(&origin);
        assert_eq!(restored.snapshot().unwrap().client_id, client);
        assert!(restored.discover(now()).is_err());
        restored.close().unwrap();
    })
    .await
    .unwrap();
    assert_eq!(server.faults.signouts.load(Ordering::SeqCst), 1);
    server.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_redemption_acknowledgement_is_inspected_and_revoked_without_replay() {
    let mut server = Server::start().await;
    let origin = server.origin.clone();
    let (device, mut session, waiting) = tokio::task::spawn_blocking(move || {
        let device = Device::new();
        let mut session = device.session(&origin);
        let waiting = session.begin("Lost reply", now()).unwrap();
        (device, session, waiting)
    })
    .await
    .unwrap();
    server.approve(waiting.connection_id.unwrap()).await;
    server.faults.lose.store(1, Ordering::SeqCst);
    let origin = server.origin.clone();
    tokio::task::spawn_blocking(move || {
        assert!(session.redeem(now()).is_err());
        assert_eq!(session.snapshot().unwrap().phase, AccountPhase::Redeeming);
        session.close().unwrap();
        let mut restored = device.session(&origin);
        assert!(restored.redeem(now()).is_err());
        assert!(restored.abandon_unissued(i64::MAX).is_err());
        assert_eq!(
            restored.inspect().unwrap().credential_status,
            DiscoveryCredentialStatus::Active
        );
        assert_eq!(
            restored.disconnect(now()).unwrap().phase,
            AccountPhase::Disconnected
        );
        assert_eq!(
            restored
                .begin("Replacement after revocation", now())
                .unwrap()
                .client_id,
            waiting.client_id
        );
        restored.disconnect(now()).unwrap();
        restored.close().unwrap();
    })
    .await
    .unwrap();
    assert_eq!(server.faults.redemptions.load(Ordering::SeqCst), 1);
    assert_eq!(server.faults.cancellations.load(Ordering::SeqCst), 2);
    server.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_sign_out_acknowledgement_is_resolved_before_any_repeat_mutation() {
    let mut server = Server::start().await;
    let origin = server.origin.clone();
    let (device, mut session, waiting) = tokio::task::spawn_blocking(move || {
        let device = Device::new();
        let mut session = device.session(&origin);
        let waiting = session.begin("Sign out", now()).unwrap();
        (device, session, waiting)
    })
    .await
    .unwrap();
    server.approve(waiting.connection_id.unwrap()).await;
    session = tokio::task::spawn_blocking(move || {
        session.redeem(now()).unwrap();
        session
    })
    .await
    .unwrap();
    server.faults.lose.store(2, Ordering::SeqCst);
    let origin = server.origin.clone();
    tokio::task::spawn_blocking(move || {
        assert!(session.disconnect(now()).is_err());
        assert_eq!(session.snapshot().unwrap().phase, AccountPhase::SigningOut);
        session.close().unwrap();
        let mut restored = device.session(&origin);
        assert_eq!(
            restored.inspect().unwrap().credential_status,
            DiscoveryCredentialStatus::Revoked
        );
        assert_eq!(
            restored.disconnect(now()).unwrap().phase,
            AccountPhase::Disconnected
        );
        restored.close().unwrap();
    })
    .await
    .unwrap();
    assert_eq!(server.faults.signouts.load(Ordering::SeqCst), 1);
    assert_eq!(server.faults.cancellations.load(Ordering::SeqCst), 0);
    server.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_initiation_and_uncertain_connected_custody_retain_the_original_identity() {
    let mut server = Server::start().await;
    let origin = server.origin.clone();
    server.faults.lose.store(3, Ordering::SeqCst);
    let (device, mut session, waiting) = tokio::task::spawn_blocking(move || {
        let device = Device::new();
        let mut session = device.session(&origin);
        let started = now();
        let client = session.snapshot().unwrap().client_id;
        assert!(session.begin("Orphaned initiation", started).is_err());
        assert_eq!(session.snapshot().unwrap().phase, AccountPhase::Initiating);
        assert_eq!(
            session.snapshot().unwrap().abandon_after,
            Some(started + 670_000)
        );
        assert!(session.begin("Blind repeat", started).is_err());
        assert!(session.abandon_unissued(started + 669_999).is_err());
        assert_eq!(
            session
                .abandon_unissued(started + 670_000)
                .unwrap()
                .client_id,
            client
        );
        let waiting = session.begin("New admitted request", now()).unwrap();
        // Two writes precede the lost initiation response, then abandonment,
        // three successful initiation writes and two redemption writes.
        device.custody.fail_write.store(8, Ordering::SeqCst);
        (device, session, waiting)
    })
    .await
    .unwrap();
    server.approve(waiting.connection_id.unwrap()).await;
    tokio::task::spawn_blocking(move || {
        assert!(session.redeem(now()).is_err());
        assert!(session.custody_pending());
        assert!(session.close().is_err());
        assert!(session.discover(now()).is_err());
        assert!(session.redeem(now()).is_err());
        assert_eq!(
            session.retry_custody().unwrap().phase,
            AccountPhase::Connected
        );
        assert_eq!(session.snapshot().unwrap().client_id, waiting.client_id);
        assert_eq!(session.discover(now()).unwrap().len(), 1);
        session.disconnect(now()).unwrap();
        session.close().unwrap();
        drop(device);
    })
    .await
    .unwrap();
    assert_eq!(server.faults.initiations.load(Ordering::SeqCst), 2);
    assert_eq!(server.faults.redemptions.load(Ordering::SeqCst), 1);
    server.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_cancellation_acknowledgement_does_not_replay_a_confirmed_mutation() {
    let mut server = Server::start().await;
    let origin = server.origin.clone();
    let (device, mut session) = tokio::task::spawn_blocking(move || {
        let device = Device::new();
        let mut session = device.session(&origin);
        session.begin("Cancelled handoff", now()).unwrap();
        (device, session)
    })
    .await
    .unwrap();
    server.faults.lose.store(4, Ordering::SeqCst);
    let origin = server.origin.clone();
    tokio::task::spawn_blocking(move || {
        assert!(session.disconnect(now()).is_err());
        assert_eq!(session.snapshot().unwrap().phase, AccountPhase::SigningOut);
        session.close().unwrap();
        let mut restored = device.session(&origin);
        assert_eq!(
            restored.disconnect(now()).unwrap().phase,
            AccountPhase::Disconnected
        );
        restored.close().unwrap();
    })
    .await
    .unwrap();
    assert_eq!(server.faults.cancellations.load(Ordering::SeqCst), 1);
    assert_eq!(server.faults.redemptions.load(Ordering::SeqCst), 0);
    server.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retained_account_owner_completes_detached_handoff_and_recovers_lost_redemption() {
    use lapis_app_core::sync::account::owner::{
        AccountCommand, AccountObservation, AccountOwner, AccountReply,
    };
    let mut server = Server::start().await;
    let origin = server.origin.clone();
    let (device, owner) = tokio::task::spawn_blocking(move || {
        let device = Device::new();
        let owner = AccountOwner::new(device.session(&origin));
        (device, owner)
    })
    .await
    .unwrap();
    let mut observations = owner.subscribe();
    drop(
        owner
            .submit(AccountCommand::Begin {
                name: "Detached Desktop".into(),
                now: now(),
            })
            .unwrap(),
    );
    let waiting = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            observations.changed().await.unwrap();
            if let AccountObservation::Ready { snapshot } = &*observations.borrow_and_update()
                && snapshot.phase == AccountPhase::AwaitingApproval
            {
                break snapshot.clone();
            }
        }
    })
    .await
    .unwrap();
    server.approve(waiting.connection_id.unwrap()).await;
    server.faults.lose.store(1, Ordering::SeqCst);
    assert!(
        owner
            .submit(AccountCommand::Redeem { now: now() })
            .unwrap()
            .wait()
            .await
            .is_err()
    );
    let AccountObservation::Ready { snapshot } = observations.borrow().clone() else {
        panic!("durable redemption intent")
    };
    assert_eq!(snapshot.phase, AccountPhase::Redeeming);
    assert!(
        owner
            .submit(AccountCommand::Redeem { now: now() })
            .unwrap()
            .wait()
            .await
            .is_err()
    );
    let AccountReply::Inspection(inspection) = owner
        .submit(AccountCommand::Inspect)
        .unwrap()
        .wait()
        .await
        .unwrap()
    else {
        panic!("inspection")
    };
    assert_eq!(
        inspection.credential_status,
        DiscoveryCredentialStatus::Active
    );
    owner
        .submit(AccountCommand::Disconnect { now: now() })
        .unwrap()
        .wait()
        .await
        .unwrap();
    owner
        .submit(AccountCommand::Close)
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(*observations.borrow(), AccountObservation::Closed);
    let origin = server.origin.clone();
    tokio::task::spawn_blocking(move || {
        let mut restored = device.session(&origin);
        let snapshot = restored.snapshot().unwrap();
        assert_eq!(snapshot.client_id, waiting.client_id);
        assert_eq!(snapshot.phase, AccountPhase::Disconnected);
        restored.close().unwrap();
    })
    .await
    .unwrap();
    assert_eq!(server.faults.initiations.load(Ordering::SeqCst), 1);
    assert_eq!(server.faults.redemptions.load(Ordering::SeqCst), 1);
    assert_eq!(server.faults.cancellations.load(Ordering::SeqCst), 1);
    server.finish().await;
}
