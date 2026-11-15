mod credentials;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use credentials::{HASH, NOW, PASSWORD};
use lapis_server::control::{
    ACCOUNT_CONNECTION_LIFETIME_MS, AccountConnectionDecision, AccountConnectionStatus, Control,
    DiscoveryCredentialStatus,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;
const PROOF: &str = "native-recovery-proof-012345678901234567890123";
const WRONG: &str = "foreign-recovery-proof-01234567890123456789012";
async fn fixture() -> (tempfile::TempDir, Control, Uuid) {
    let root = tempfile::tempdir().unwrap();
    let control = Control::open(root.path()).await.unwrap();
    control.initialize("Owner", HASH, NOW).await.unwrap();
    let session = control
        .login("owner", PASSWORD, "recovery-test", NOW)
        .await
        .unwrap()
        .session
        .id;
    (root, control, session)
}
#[tokio::test]
async fn lost_redemption_is_inspectable_and_revocable_after_request_expiry_and_restart() {
    if let Some(path) = std::env::var_os("LAPIS_SYNC_RECOVERY_COLD_PROFILE") {
        let id = Uuid::parse_str(&std::env::var("LAPIS_SYNC_RECOVERY_COLD_ID").unwrap()).unwrap();
        let later = NOW + ACCOUNT_CONNECTION_LIFETIME_MS + 1;
        let control = Control::open(std::path::Path::new(&path)).await.unwrap();
        assert_eq!(
            control
                .inspect_native_account_connection(id, PROOF, later)
                .await
                .unwrap()
                .credential_status,
            DiscoveryCredentialStatus::Active
        );
        assert_eq!(
            control
                .cancel_native_account_connection(id, PROOF, later)
                .await
                .unwrap()
                .credential_status,
            DiscoveryCredentialStatus::Revoked
        );
        control.close().await.unwrap();
        return;
    }
    let (root, control, session) = fixture().await;
    let client = Uuid::new_v4();
    let request = control
        .create_account_connection(
            client,
            "Desktop",
            &URL_SAFE_NO_PAD.encode(Sha256::digest(PROOF)),
            NOW,
        )
        .await
        .unwrap();
    control
        .decide_account_connection(
            request.id,
            session,
            AccountConnectionDecision::Approve,
            NOW + 1,
        )
        .await
        .unwrap();
    let issued = control
        .redeem_account_connection(request.id, PROOF, NOW + 2)
        .await
        .unwrap();
    assert!(
        control
            .inspect_native_account_connection(request.id, WRONG, NOW + 3)
            .await
            .is_err()
    );
    let observed = control
        .inspect_native_account_connection(request.id, PROOF, NOW + 3)
        .await
        .unwrap();
    assert_eq!(observed.client_id, client);
    assert_eq!(observed.status, AccountConnectionStatus::Consumed);
    assert_eq!(
        observed.credential_status,
        DiscoveryCredentialStatus::Active
    );
    control.close().await.unwrap();
    drop(control);
    let control = Control::open(root.path()).await.unwrap();
    let later = NOW + ACCOUNT_CONNECTION_LIFETIME_MS + 1;
    // Trigger ordinary request expiry cleanup without discarding the issued association.
    control
        .create_account_connection(
            Uuid::new_v4(),
            "New request",
            &URL_SAFE_NO_PAD.encode(Sha256::digest(WRONG)),
            later,
        )
        .await
        .unwrap();
    let observed = control
        .inspect_native_account_connection(request.id, PROOF, later)
        .await
        .unwrap();
    assert_eq!(
        observed.credential_status,
        DiscoveryCredentialStatus::Active
    );
    assert!(
        control
            .cancel_native_account_connection(request.id, WRONG, later)
            .await
            .is_err()
    );
    assert!(
        control
            .resolve_account_credential(&issued.token, later)
            .await
            .unwrap()
            .is_some()
    );
    control.close().await.unwrap();
    drop(control);
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "lost_redemption_is_inspectable_and_revocable_after_request_expiry_and_restart",
            "--nocapture",
        ])
        .env("LAPIS_SYNC_RECOVERY_COLD_PROFILE", root.path())
        .env("LAPIS_SYNC_RECOVERY_COLD_ID", request.id.to_string())
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "cold recovery process failed: {}",
        String::from_utf8_lossy(&child.stderr)
    );
    let control = Control::open(root.path()).await.unwrap();
    assert!(
        control
            .resolve_account_credential(&issued.token, later + 1)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        control
            .cancel_native_account_connection(request.id, PROOF, later + 2)
            .await
            .unwrap()
            .credential_status,
        DiscoveryCredentialStatus::Revoked
    );
}

#[tokio::test]
async fn cancellation_before_owner_decision_or_redemption_prevents_credential_issuance() {
    let (_root, control, session) = fixture().await;
    for approved in [false, true] {
        let request = control
            .create_account_connection(
                Uuid::new_v4(),
                "Desktop",
                &URL_SAFE_NO_PAD.encode(Sha256::digest(PROOF)),
                NOW,
            )
            .await
            .unwrap();
        if approved {
            control
                .decide_account_connection(
                    request.id,
                    session,
                    AccountConnectionDecision::Approve,
                    NOW + 1,
                )
                .await
                .unwrap();
        }
        let outcome = control
            .cancel_native_account_connection(request.id, PROOF, NOW + 2)
            .await
            .unwrap();
        assert_eq!(outcome.status, AccountConnectionStatus::Rejected);
        assert_eq!(
            outcome.credential_status,
            DiscoveryCredentialStatus::NotIssued
        );
        assert!(
            control
                .redeem_account_connection(request.id, PROOF, NOW + 3)
                .await
                .is_err()
        );
        assert!(
            control
                .decide_account_connection(
                    request.id,
                    session,
                    AccountConnectionDecision::Approve,
                    NOW + 3
                )
                .await
                .is_err()
        );
        assert_eq!(
            control
                .inspect_account_connection(request.id, NOW + 3)
                .await
                .unwrap()
                .unwrap()
                .status,
            AccountConnectionStatus::Rejected
        );
    }
}
#[tokio::test]
async fn cancellation_and_redemption_serialize_without_revoking_an_unrelated_credential() {
    let (_root, control, session) = fixture().await;
    let mut requests = Vec::new();
    for name in ["Racing", "Unrelated"] {
        let request = control
            .create_account_connection(
                Uuid::new_v4(),
                name,
                &URL_SAFE_NO_PAD.encode(Sha256::digest(PROOF)),
                NOW,
            )
            .await
            .unwrap();
        control
            .decide_account_connection(
                request.id,
                session,
                AccountConnectionDecision::Approve,
                NOW + 1,
            )
            .await
            .unwrap();
        requests.push(request.id);
    }
    let unrelated = control
        .redeem_account_connection(requests[1], PROOF, NOW + 2)
        .await
        .unwrap();
    let (issued, cancelled) = tokio::join!(
        control.redeem_account_connection(requests[0], PROOF, NOW + 3),
        control.cancel_native_account_connection(requests[0], PROOF, NOW + 3)
    );
    cancelled.unwrap();
    if let Ok(issued) = issued {
        assert!(
            control
                .resolve_account_credential(&issued.token, NOW + 4)
                .await
                .unwrap()
                .is_none()
        );
    }
    assert!(
        control
            .redeem_account_connection(requests[0], PROOF, NOW + 5)
            .await
            .is_err()
    );
    assert!(
        control
            .resolve_account_credential(&unrelated.token, NOW + 5)
            .await
            .unwrap()
            .is_some()
    );
}
#[tokio::test]
async fn legacy_consumed_records_without_associations_are_explicitly_unavailable() {
    let (root, control, session) = fixture().await;
    let request = control
        .create_account_connection(
            Uuid::new_v4(),
            "Historical",
            &URL_SAFE_NO_PAD.encode(Sha256::digest(PROOF)),
            NOW,
        )
        .await
        .unwrap();
    control
        .decide_account_connection(
            request.id,
            session,
            AccountConnectionDecision::Approve,
            NOW + 1,
        )
        .await
        .unwrap();
    let issued = control
        .redeem_account_connection(request.id, PROOF, NOW + 2)
        .await
        .unwrap();
    control.close().await.unwrap();
    drop(control);
    // Retain old records and hashes while reproducing the pre-association schema.
    let db = turso::Builder::new_local(root.path().join("control/auth.turso").to_str().unwrap())
        .build()
        .await
        .unwrap();
    let sql = db.connect().unwrap();
    sql.execute("DROP TABLE sync_account_outcomes", ())
        .await
        .unwrap();
    drop(sql);
    drop(db);
    let control = Control::open(root.path()).await.unwrap();
    let inspected = control
        .inspect_native_account_connection(request.id, PROOF, NOW + 3)
        .await
        .unwrap();
    assert_eq!(inspected.status, AccountConnectionStatus::Consumed);
    assert_eq!(
        inspected.credential_status,
        DiscoveryCredentialStatus::Unavailable
    );
    assert!(
        control
            .cancel_native_account_connection(request.id, PROOF, NOW + 3)
            .await
            .is_err()
    );
    assert!(
        control
            .resolve_account_credential(&issued.token, NOW + 4)
            .await
            .unwrap()
            .is_some()
    );
}
