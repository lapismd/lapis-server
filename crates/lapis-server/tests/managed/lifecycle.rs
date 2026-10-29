use super::Fixture;
use lapis_app_protocol::{ExpectedVersion, Outcome, VaultCommand, VaultValue};
use serde_json::json;

#[tokio::test]
async fn archive_guards_dirty_editors_fences_old_clients_and_restores_original_bytes() {
    let mut f = Fixture::start().await;
    let vault = f.create("Archive me").await;
    let id = vault.vault_id;
    let client = f.attach(id).await;
    f.execute(
        &client,
        VaultCommand::WriteText {
            path: "Note.md".into(),
            text: "retained bytes".into(),
            expected: ExpectedVersion::Missing {},
        },
    )
    .await;
    let opened = f
        .execute(
            &client,
            VaultCommand::OpenEditor {
                path: "Note.md".into(),
            },
        )
        .await;
    let Outcome::Ok {
        value: VaultValue::Editor { editor_id },
    } = opened.outcome
    else {
        panic!("editor was not admitted")
    };
    f.execute(
        &client,
        VaultCommand::MarkDirty {
            editor_id,
            revision: 1,
        },
    )
    .await;
    let refused = f
        .post(&format!("/api/v3/vaults/{id}/archive"))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 409);
    assert!(
        f.list()
            .await
            .iter()
            .find(|row| row.vault_id == id)
            .unwrap()
            .archived_at
            .is_none()
    );
    assert!(matches!(
        f.execute(
            &client,
            VaultCommand::ReadText {
                path: "Note.md".into()
            }
        )
        .await
        .outcome,
        Outcome::Ok { .. }
    ));
    f.execute(&client, VaultCommand::DiscardEditor { editor_id })
        .await;
    let archived = f.action(id, "archive").await;
    assert!(archived.archived_at.is_some());
    assert_eq!(
        f.action(id, "archive").await.archived_at,
        archived.archived_at
    );
    assert_eq!(
        f.post(&format!("/api/v3/vaults/{id}/clients"))
            .json(&json!({"protocol_version":3}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.command(&client, VaultCommand::Status {})
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        std::fs::read_to_string(f.profile.join(format!("vaults/{id}/content/Note.md"))).unwrap(),
        "retained bytes"
    );
    assert!(f.action(id, "restore").await.archived_at.is_none());
    assert!(f.action(id, "restore").await.archived_at.is_none());
    let restored = f.attach(id).await;
    assert_ne!(restored.generation, client.generation);
    assert!(
        matches!(f.execute(&restored, VaultCommand::ReadText { path: "Note.md".into() }).await.outcome, Outcome::Ok { value: VaultValue::Text { snapshot } } if snapshot.text == "retained bytes")
    );
    f.finish().await;
}

#[tokio::test]
async fn profile_backed_startup_supports_an_empty_installation_without_changing_token_staging() {
    use lapis_server::{config::Config, control::Control, host::Host};
    use std::sync::Arc;
    let profile = tempfile::tempdir().unwrap();
    assert!(Host::new(Config { vaults: vec![] }).is_err());
    let control = Arc::new(Control::open(profile.path()).await.unwrap());
    control
        .initialize("owner", crate::credentials::HASH, crate::credentials::NOW)
        .await
        .unwrap();
    let host = Host::with_profile(Config { vaults: vec![] }, profile.path()).unwrap();
    host.enable_management(control.clone(), profile.path())
        .await
        .unwrap();
    assert!(host.manages_vaults());
    assert!(host.vaults().unwrap().is_empty());
    host.shutdown().unwrap();
    control.close().await.unwrap();
}
