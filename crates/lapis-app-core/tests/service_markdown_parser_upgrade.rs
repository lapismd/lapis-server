use lapis_app_core::{
    database::Database,
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime, VaultService},
};
use lapis_app_protocol::database::{
    call::{DatabaseCall, DatabaseMethod, DatabaseValue},
    indexed::records::CoreIndexedFile,
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

async fn close(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
async fn indexed(vault: &Arc<VaultService>, path: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        while vault.indexing_status().unwrap().passes == 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("native metadata pass must complete");
    let client = vault.attach().unwrap();
    match client
        .call_database(DatabaseCall {
            method: DatabaseMethod::GetIndexedFile,
            args: vec![DatabaseValue::Value { value: json!(path) }],
        })
        .await
        .unwrap()
    {
        DatabaseValue::Value { value } => value,
        DatabaseValue::Undefined {} => panic!("native metadata must exist"),
    }
}
fn runtime(profile: &tempfile::TempDir) -> Arc<Runtime> {
    Arc::new(Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    ))
}

#[tokio::test]
async fn cold_parser_identity_upgrade_reparses_unchanged_native_metadata() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Note.md");
    let source = "---\nvalue: retained\n---\n# Note 📝\n";
    std::fs::write(&path, source).unwrap();
    let edge = "---\nvalue: alpha\u{85}beta\n---\n# YAML edge\n";
    std::fs::write(root.path().join("Edge.md"), edge).unwrap();
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    let id = Uuid::new_v4();
    let first = runtime(&profile);
    let vault = first.admit(id, root.path()).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    let before = indexed(&vault, "Note.md").await;
    close(first).await;

    // Keep the exact saved-file hash/stat and normalized property/link rows.
    // Only the disposable parser identity/metadata come from the prior cache.
    let database = Database::open(profile.path(), id).await.unwrap();
    let cached = database.get_indexed_file("Note.md").await.unwrap().unwrap();
    let mut metadata = cached.metadata.unwrap();
    metadata.parser_version = "markdown-native-v1".into();
    metadata.metadata["frontmatter"]["value"] = json!("stale prior parser metadata");
    database
        .upsert_indexed_metadata(CoreIndexedFile {
            file: cached.file,
            metadata,
            links: cached.links,
            tags: cached.tags,
            properties: cached.properties,
        })
        .await
        .unwrap();
    database.close().await.unwrap();

    let next = runtime(&profile);
    let vault = next.admit(id, root.path()).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    let after = indexed(&vault, "Note.md").await;
    let edge_after = indexed(&vault, "Edge.md").await;
    close(next).await;
    assert_eq!(after["metadata"]["parserVersion"], "markdown-native-v2");
    assert_eq!(
        after["metadata"]["metadata"]["frontmatter"]["value"],
        "retained"
    );
    assert_eq!(
        edge_after["metadata"]["metadata"]["frontmatter"]["value"],
        "alpha\u{85}beta"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("Edge.md")).unwrap(),
        edge
    );
    assert_eq!(after["file"]["hash"], before["file"]["hash"]);
    assert_eq!(after["file"]["mtime"], before["file"]["mtime"]);
    assert_eq!(after["file"]["size"], before["file"]["size"]);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        modified
    );
}
