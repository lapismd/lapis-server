use super::support::Fixture;
use lapis_app_protocol::{ErrorCode, ExpectedVersion, Outcome, VaultCommand, VaultValue};

#[tokio::test]
async fn detached_dirty_editor_blocks_other_writers_and_quit_until_explicit_recovery() {
    let f = Fixture::start().await;
    let owner = f.attach(f.ids[0]).await;
    let editor = match f
        .execute(
            &owner,
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
    let dirty = f
        .execute(
            &owner,
            VaultCommand::MarkDirty {
                editor_id: editor,
                revision: 1,
            },
        )
        .await;
    assert!(matches!(dirty.outcome, Outcome::Ok { .. }));
    f.detach(&owner).await;
    let attached = f.attach(f.ids[0]).await;
    assert_eq!(attached.generation, owner.generation);
    let claims = f.execute(&attached, VaultCommand::EditorClaims {}).await;
    assert!(
        matches!(claims.outcome, Outcome::Ok { value: VaultValue::EditorClaims { claims } } if claims.len() == 1 && claims[0].detached)
    );
    let write = || VaultCommand::WriteText {
        path: "draft.md".into(),
        text: "persisted".into(),
        expected: ExpectedVersion::Missing {},
    };
    let denied = f.execute(&attached, write()).await;
    assert!(matches!(denied.outcome, Outcome::Error { error } if error.code == ErrorCode::Busy));
    let close = f
        .authorize(
            f.client
                .delete(format!("{}/api/v3/vaults/{}", f.base, f.ids[0])),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(close.status(), 409);
    assert_eq!(f.host.shutdown().unwrap_err().code, ErrorCode::Busy);
    let recovered = f
        .execute(&attached, VaultCommand::RecoverEditor { editor_id: editor })
        .await;
    assert!(matches!(recovered.outcome, Outcome::Ok { .. }));
    let discarded = f
        .execute(&attached, VaultCommand::DiscardEditor { editor_id: editor })
        .await;
    assert!(matches!(discarded.outcome, Outcome::Ok { .. }));
    assert!(matches!(
        f.execute(&attached, write()).await.outcome,
        Outcome::Ok { .. }
    ));
    f.detach(&attached).await;
    let next = f.attach(f.ids[0]).await;
    assert!(
        matches!(f.execute(&next, VaultCommand::ReadText { path: "draft.md".into() }).await.outcome, Outcome::Ok { value: VaultValue::Text { snapshot } } if snapshot.text == "persisted")
    );
    f.detach(&next).await;
    f.host.shutdown().unwrap();
}

#[tokio::test]
async fn competing_http_clients_cannot_publish_against_the_same_previous_version() {
    let f = Fixture::start().await;
    let a = f.attach(f.ids[0]).await;
    let b = f.attach(f.ids[0]).await;
    let write = |text: &str| VaultCommand::WriteText {
        path: "race.md".into(),
        text: text.into(),
        expected: ExpectedVersion::Missing {},
    };
    let (a, b) = tokio::join!(
        f.execute(&a, write("first")),
        f.execute(&b, write("second"))
    );
    let replies = [a, b];
    assert_eq!(
        replies
            .iter()
            .filter(|reply| matches!(reply.outcome, Outcome::Ok { .. }))
            .count(),
        1
    );
    assert_eq!(replies.iter().filter(|reply| matches!(&reply.outcome, Outcome::Error { error } if error.code == ErrorCode::Conflict)).count(), 1);
    let stored = std::fs::read_to_string(f.roots[0].path().join("race.md")).unwrap();
    assert!(stored == "first" || stored == "second");
    f.host.shutdown().unwrap();
}
