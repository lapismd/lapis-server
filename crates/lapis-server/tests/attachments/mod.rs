use super::support::Fixture;
use lapis_app_protocol::{ErrorCode, ExpectedVersion, Outcome, VaultCommand, VaultValue};
use lapis_server::host::CLIENT_IDLE_TIMEOUT;
use std::time::{Duration, Instant};

#[tokio::test]
async fn heartbeat_keeps_the_original_attachment_and_requires_authentication() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    let url = format!("{}/api/v3/clients/{}/heartbeat", f.base, client.client_id);
    assert_eq!(f.client.post(&url).send().await.unwrap().status(), 401);
    let reply = f.authorize(f.client.post(&url)).send().await.unwrap();
    assert_eq!(reply.status(), 204);
    f.detach(&client).await;
    assert_eq!(
        f.authorize(f.client.post(&url))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    f.host.shutdown().unwrap();
}

#[tokio::test]
async fn heartbeat_renews_the_deadline_and_expiry_cannot_be_reversed() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    let prior = Instant::now();
    let url = format!("{}/api/v3/clients/{}/heartbeat", f.base, client.client_id);
    assert_eq!(
        f.authorize(f.client.post(&url))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    // This deadline is past the old lease but before the completed heartbeat's.
    assert_eq!(f.host.expire_idle(prior + CLIENT_IDLE_TIMEOUT).unwrap(), 0);
    assert_eq!(
        f.host
            .expire_idle(Instant::now() + CLIENT_IDLE_TIMEOUT + Duration::from_secs(1))
            .unwrap(),
        1
    );
    assert_eq!(
        f.authorize(f.client.post(&url))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let fresh = f.attach(f.ids[0]).await;
    assert_eq!(fresh.generation, client.generation);
    assert_ne!(fresh.client_id, client.client_id);
    f.detach(&fresh).await;
    f.host.shutdown().unwrap();
}

#[tokio::test]
async fn expiry_detaches_dirty_claims_without_closing_the_vault_or_granting_write_access() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    let editor_id = match f
        .execute(
            &client,
            VaultCommand::OpenEditor {
                path: "draft.md".into(),
            },
        )
        .await
        .outcome
    {
        Outcome::Ok {
            value: VaultValue::Editor { editor_id },
        } => editor_id,
        other => panic!("{other:?}"),
    };
    assert!(matches!(
        f.execute(
            &client,
            VaultCommand::MarkDirty {
                editor_id,
                revision: 1
            }
        )
        .await
        .outcome,
        Outcome::Ok { .. }
    ));
    assert_eq!(
        f.host
            .expire_idle(Instant::now() + CLIENT_IDLE_TIMEOUT)
            .unwrap(),
        1
    );
    let fresh = f.attach(f.ids[0]).await;
    assert_eq!(fresh.generation, client.generation);
    let claims = f.execute(&fresh, VaultCommand::EditorClaims {}).await;
    assert!(
        matches!(claims.outcome, Outcome::Ok { value: VaultValue::EditorClaims { claims } } if claims.len() == 1 && claims[0].detached)
    );
    let write = f
        .execute(
            &fresh,
            VaultCommand::WriteText {
                path: "draft.md".into(),
                text: "unsafe".into(),
                expected: ExpectedVersion::Missing {},
            },
        )
        .await;
    assert!(matches!(write.outcome, Outcome::Error { error } if error.code == ErrorCode::Busy));
    assert_eq!(f.host.shutdown().unwrap_err().code, ErrorCode::Busy);
    assert!(matches!(
        f.execute(&fresh, VaultCommand::RecoverEditor { editor_id })
            .await
            .outcome,
        Outcome::Ok { .. }
    ));
    assert!(matches!(
        f.execute(&fresh, VaultCommand::DiscardEditor { editor_id })
            .await
            .outcome,
        Outcome::Ok { .. }
    ));
    f.detach(&fresh).await;
    f.host.shutdown().unwrap();
}

#[tokio::test]
async fn expired_attachments_release_the_native_client_limit() {
    let f = Fixture::start().await;
    let mut generation = None;
    for _ in 0..128 {
        generation = Some(
            f.host
                .attach(f.ids[0], lapis_server::host::Principal::Token)
                .unwrap()
                .generation,
        );
    }
    assert_eq!(
        f.host
            .attach(f.ids[0], lapis_server::host::Principal::Token)
            .unwrap_err()
            .code,
        ErrorCode::Busy
    );
    assert_eq!(
        f.host
            .expire_idle(Instant::now() + CLIENT_IDLE_TIMEOUT)
            .unwrap(),
        128
    );
    let fresh = f.attach(f.ids[0]).await;
    assert_eq!(Some(fresh.generation), generation);
    f.detach(&fresh).await;
    f.host.shutdown().unwrap();
}
