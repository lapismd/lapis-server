mod credentials;
mod managed;

use lapis_app_protocol::{ClientDescriptor, ExpectedVersion, ManagedVault, Outcome, VaultCommand};
use managed::Fixture;
use serde_json::json;

#[tokio::test]
async fn creates_a_host_owned_vault_and_admits_its_existing_file_service() {
    let mut f = Fixture::start().await;
    let response = f
        .post("/api/v3/vaults")
        .json(&json!({ "name": "New 笔记" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        201,
        "the retained chooser can create a Rust-hosted vault"
    );
    let created: ManagedVault = response.json().await.unwrap();
    let id = created.vault_id;
    assert_eq!(created.name, "New 笔记");
    let content = f.profile.join(format!("vaults/{id}/content"));
    assert!(content.is_dir());
    assert!(!f.profile.join("New 笔记").exists());
    let admitted = f
        .post(&format!("/api/v3/vaults/{id}/clients"))
        .json(&json!({ "protocol_version": 3 }))
        .send()
        .await
        .unwrap();
    assert_eq!(admitted.status(), 200);
    let client: ClientDescriptor = admitted.json().await.unwrap();
    let reply = f
        .execute(
            &client,
            VaultCommand::WriteText {
                path: "Note.md".into(),
                text: "# Created through Rust\n".into(),
                expected: ExpectedVersion::Missing {},
            },
        )
        .await;
    assert!(matches!(reply.outcome, Outcome::Ok { .. }));
    assert_eq!(
        std::fs::read_to_string(content.join("Note.md")).unwrap(),
        "# Created through Rust\n"
    );
    f.finish().await;
}
