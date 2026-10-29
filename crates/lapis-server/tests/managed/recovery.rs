use super::Fixture;
use serde_json::json;

#[tokio::test]
async fn renamed_configured_and_archived_created_vaults_survive_a_cold_reopen() {
    let f = Fixture::start().await;
    let configured = f.configured;
    assert_eq!(
        f.post(&format!("/api/v3/vaults/{configured}/name"))
            .json(&json!({"name":"Configured renamed"}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let created = f.create("Created").await;
    let id = created.vault_id;
    let content = f.profile.join(format!("vaults/{id}/content"));
    std::fs::write(content.join("Note.md"), "cold bytes").unwrap();
    assert_eq!(
        f.post(&format!("/api/v3/vaults/{id}/name"))
            .json(&json!({"name":"Created renamed"}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    f.action(id, "archive").await;
    let mut f = f.restart().await;
    let rows = f.list().await;
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .find(|row| row.vault_id == configured)
            .unwrap()
            .name,
        "Configured renamed"
    );
    let restored = rows.iter().find(|row| row.vault_id == id).unwrap();
    assert_eq!(restored.name, "Created renamed");
    assert!(restored.archived_at.is_some());
    f.action(id, "restore").await;
    assert_eq!(
        std::fs::read_to_string(content.join("Note.md")).unwrap(),
        "cold bytes"
    );
    f.attach(id).await;
    f.finish().await;
}

#[tokio::test]
async fn restore_rejects_a_replaced_physical_root_and_startup_rejects_its_changed_identity() {
    use lapis_server::{config::Config, control::Control, host::Host};
    use std::sync::Arc;
    let mut f = Fixture::start().await;
    let id = f.create("Physical identity").await.vault_id;
    f.action(id, "archive").await;
    let content = f.profile.join(format!("vaults/{id}/content"));
    let original = content.with_file_name("original-content");
    std::fs::write(content.join("Preserved.md"), "original").unwrap();
    std::fs::rename(&content, &original).unwrap();
    std::fs::create_dir(&content).unwrap();
    assert!(
        !f.post(&format!("/api/v3/vaults/{id}/restore"))
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    assert!(content.read_dir().unwrap().next().is_none());
    assert_eq!(
        std::fs::read_to_string(original.join("Preserved.md")).unwrap(),
        "original"
    );
    f.finish().await;
    let control = Arc::new(Control::open(&f.profile).await.unwrap());
    let host = Host::with_profile(Config { vaults: vec![] }, &f.profile).unwrap();
    let result = host.enable_management(control.clone(), &f.profile).await;
    assert!(result.unwrap_err().to_string().contains("storage changed"));
    host.shutdown().unwrap();
    control.close().await.unwrap();
}
