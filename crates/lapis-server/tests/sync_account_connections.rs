use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use lapis_server::control::{
    ACCOUNT_CONNECTION_LIFETIME_MS, ACCOUNT_CREDENTIAL_LIFETIME_MS, AccountConnectionDecision,
    Control,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

mod credentials;
use credentials::{HASH, NOW, PASSWORD};

fn verifier() -> &'static str {
    "account-connection-verifier-0123456789abcdefghi"
}

fn challenge() -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier().as_bytes()))
}

async fn fixture() -> (tempfile::TempDir, Control, Uuid) {
    let root = tempfile::tempdir().unwrap();
    let control = Control::open(root.path()).await.unwrap();
    control.initialize("Owner", HASH, NOW).await.unwrap();
    let session = control
        .login("owner", PASSWORD, "sync-test", NOW)
        .await
        .unwrap()
        .session
        .id;
    (root, control, session)
}

#[tokio::test]
async fn approved_s256_request_issues_one_hashed_revocable_discovery_credential() {
    let (root, control, session) = fixture().await;
    let client = Uuid::new_v4();
    let request = control
        .create_account_connection(client, "Desktop", &challenge(), NOW + 1)
        .await
        .unwrap();
    control
        .decide_account_connection(
            request.id,
            session,
            AccountConnectionDecision::Approve,
            NOW + 2,
        )
        .await
        .unwrap();
    let issued = control
        .redeem_account_connection(request.id, verifier(), NOW + 3)
        .await
        .unwrap();
    assert_eq!(issued.credential.client_id, client);
    assert_eq!(
        issued.credential.expires_at,
        NOW + 3 + ACCOUNT_CREDENTIAL_LIFETIME_MS
    );
    assert!(
        control
            .resolve_account_credential(&issued.token, NOW + 4)
            .await
            .unwrap()
            .is_some()
    );
    let credential_id = issued.credential.id;
    let token = issued.token;
    control.close().await.unwrap();
    drop(control);

    let reopened = Control::open(root.path()).await.unwrap();
    assert!(
        reopened
            .resolve_account_credential(&token, NOW + 5)
            .await
            .unwrap()
            .is_some()
    );
    reopened
        .revoke_account_credential(credential_id, NOW + 6)
        .await
        .unwrap();
    assert!(
        reopened
            .resolve_account_credential(&token, NOW + 7)
            .await
            .unwrap()
            .is_none()
    );
    let database =
        turso::Builder::new_local(root.path().join("control/auth.turso").to_str().unwrap())
            .build()
            .await
            .unwrap();
    let connection = database.connect().unwrap();
    let mut rows = connection
        .query(
            "SELECT token_hash FROM sync_account_credentials WHERE id=?",
            [credential_id.to_string()],
        )
        .await
        .unwrap();
    let stored = rows
        .next()
        .await
        .unwrap()
        .unwrap()
        .get::<String>(0)
        .unwrap();
    assert_ne!(stored, token);
}

#[tokio::test]
async fn wrong_expired_rejected_and_replayed_requests_fail_closed() {
    let (_root, control, session) = fixture().await;
    let request = control
        .create_account_connection(Uuid::new_v4(), "Desktop", &challenge(), NOW)
        .await
        .unwrap();
    assert!(
        control
            .decide_account_connection(
                request.id,
                Uuid::new_v4(),
                AccountConnectionDecision::Approve,
                NOW + 1
            )
            .await
            .is_err()
    );
    control
        .decide_account_connection(
            request.id,
            session,
            AccountConnectionDecision::Approve,
            NOW + 2,
        )
        .await
        .unwrap();
    assert!(
        control
            .redeem_account_connection(
                request.id,
                "wrong-verifier-012345678901234567890123456789",
                NOW + 3
            )
            .await
            .is_err()
    );
    assert!(
        control
            .resolve_account_credential(verifier(), NOW + 3)
            .await
            .unwrap()
            .is_none()
    );
    control
        .redeem_account_connection(request.id, verifier(), NOW + 4)
        .await
        .unwrap();
    assert!(
        control
            .redeem_account_connection(request.id, verifier(), NOW + 5)
            .await
            .is_err()
    );

    let expired = control
        .create_account_connection(Uuid::new_v4(), "Expired", &challenge(), NOW)
        .await
        .unwrap();
    assert!(
        control
            .decide_account_connection(
                expired.id,
                session,
                AccountConnectionDecision::Approve,
                NOW + ACCOUNT_CONNECTION_LIFETIME_MS
            )
            .await
            .is_err()
    );
    let rejected = control
        .create_account_connection(Uuid::new_v4(), "Rejected", &challenge(), NOW + 10)
        .await
        .unwrap();
    control
        .decide_account_connection(
            rejected.id,
            session,
            AccountConnectionDecision::Reject,
            NOW + 11,
        )
        .await
        .unwrap();
    assert!(
        control
            .redeem_account_connection(rejected.id, verifier(), NOW + 12)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn malformed_requests_are_rejected_before_persistence() {
    let (_root, control, _session) = fixture().await;
    assert!(
        control
            .create_account_connection(Uuid::new_v4(), "", &challenge(), NOW)
            .await
            .is_err()
    );
    assert!(
        control
            .create_account_connection(Uuid::nil(), "Desktop", &challenge(), NOW)
            .await
            .is_err()
    );
    assert!(
        control
            .create_account_connection(Uuid::new_v4(), "Desktop", "not-a-challenge", NOW)
            .await
            .is_err()
    );
    assert!(
        control
            .create_account_connection(Uuid::new_v4(), &"x".repeat(129), &challenge(), NOW)
            .await
            .is_err()
    );
}
