use lapis_app_core::{
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime, SearchIndexingConfiguration, VaultService},
};
use lapis_app_protocol::database::{
    call::{DatabaseCall, DatabaseMethod, DatabaseValue},
    search::source::SearchChunkingSettings,
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

fn runtime(profile: &tempfile::TempDir) -> Arc<Runtime> {
    Arc::new(Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    ))
}
async fn close(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
async fn row(vault: &Arc<VaultService>, path: &str) -> Option<Value> {
    let client = vault.attach().unwrap();
    match client
        .call_database(DatabaseCall {
            method: DatabaseMethod::GetSearchDocument,
            args: vec![DatabaseValue::Value { value: json!(path) }],
        })
        .await
        .unwrap()
    {
        DatabaseValue::Undefined {} => None,
        DatabaseValue::Value { value } if value.is_null() => None,
        DatabaseValue::Value { value } => Some(value),
    }
}
async fn until(mut ready: impl AsyncFnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !ready().await {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("native Search source must converge");
}
fn configuration(canvas: bool, target: f64) -> SearchIndexingConfiguration {
    SearchIndexingConfiguration {
        canvas,
        chunking: SearchChunkingSettings {
            target_chars: target,
            breakpoint_window_chars: 32.0,
            breakpoint_decay: 0.5,
        },
    }
}

#[tokio::test]
async fn configured_canvas_source_indexes_external_changes_and_prunes_only_its_owned_rows() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Diagram.canvas");
    let source = r##"{"nodes":[{"type":"text","text":"# Plan 📝"}],"edges":[{"label":"next"}]}"##;
    std::fs::write(&path, source).unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault
        .start_search_indexing_with(configuration(true, 256.0))
        .await
        .unwrap();
    until(async || row(&vault, "Diagram.canvas").await.is_some()).await;
    let first = row(&vault, "Diagram.canvas").await.unwrap();
    assert_eq!(first["sourceProviderId"], "search:canvas");
    assert_eq!(first["content"], "text\n# Plan 📝\nnext");
    assert_eq!(first["sourceMetadata"]["chunking"]["targetChars"], 256.0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(vault.status().unwrap().clients, 0);
    std::fs::write(&path, r#"{"nodes":[{"text":"external pear"}]}"#).unwrap();
    until(async || {
        row(&vault, "Diagram.canvas")
            .await
            .is_some_and(|value| value["content"] == "external pear")
    })
    .await;
    std::fs::remove_file(&path).unwrap();
    until(async || row(&vault, "Diagram.canvas").await.is_none()).await;
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
}

#[tokio::test]
async fn changing_chunking_reprepares_unchanged_markdown_and_canvas_without_source_writes() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("Note.md"),
        "# Stable\n\nunchanged source\n",
    )
    .unwrap();
    let source = r#"{"nodes":[{"text":"unchanged Canvas"}]}"#;
    std::fs::write(root.path().join("Diagram.canvas"), source).unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault
        .start_search_indexing_with(configuration(true, 256.0))
        .await
        .unwrap();
    until(async || {
        row(&vault, "Diagram.canvas").await.is_some() && row(&vault, "Note.md").await.is_some()
    })
    .await;
    let old = row(&vault, "Note.md").await.unwrap();
    vault
        .start_search_indexing_with(configuration(true, 512.0))
        .await
        .unwrap();
    until(async || {
        row(&vault, "Note.md")
            .await
            .is_some_and(|value| value["sourceMetadata"]["chunking"]["targetChars"] == 512.0)
            && row(&vault, "Diagram.canvas")
                .await
                .is_some_and(|value| value["sourceMetadata"]["chunking"]["targetChars"] == 512.0)
    })
    .await;
    let new = row(&vault, "Note.md").await.unwrap();
    assert_eq!(new["checksum"], old["checksum"]);
    assert_ne!(
        new["sourceMetadata"]["projectionSignature"],
        old["sourceMetadata"]["projectionSignature"]
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("Diagram.canvas")).unwrap(),
        source
    );
    close(runtime).await;
}

#[tokio::test]
async fn invalid_chunking_is_rejected_before_starting_an_owner_or_replacing_options() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.md"), "# Retained\n").unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let error = vault
        .start_search_indexing_with(configuration(true, 0.0))
        .await
        .unwrap_err();
    assert_eq!(error.code(), lapis_app_protocol::ErrorCode::InvalidRequest);
    assert_eq!(
        vault.indexing_status().unwrap_err().code(),
        lapis_app_protocol::ErrorCode::Unsupported
    );
    vault
        .start_search_indexing_with(configuration(true, 256.0))
        .await
        .unwrap();
    until(async || row(&vault, "Note.md").await.is_some()).await;
    let before = row(&vault, "Note.md").await.unwrap();
    let mut invalid = configuration(true, 512.0);
    invalid.chunking.breakpoint_decay = f64::NAN;
    assert!(vault.start_search_indexing_with(invalid).await.is_err());
    assert_eq!(row(&vault, "Note.md").await.unwrap(), before);
    close(runtime).await;
}

#[tokio::test]
async fn markdown_only_reselection_does_not_admit_canvas_changes() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("Diagram.canvas"),
        r#"{"nodes":[{"text":"pear"}]}"#,
    )
    .unwrap();
    std::fs::write(root.path().join("Note.md"), "# First\n").unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_search_indexing().await.unwrap();
    until(async || row(&vault, "Note.md").await.is_some()).await;
    assert!(row(&vault, "Diagram.canvas").await.is_none());
    vault
        .start_search_indexing_with(configuration(true, 256.0))
        .await
        .unwrap();
    until(async || row(&vault, "Diagram.canvas").await.is_some()).await;
    vault.start_search_indexing().await.unwrap();
    std::fs::write(
        root.path().join("Diagram.canvas"),
        r#"{"nodes":[{"text":"plum"}]}"#,
    )
    .unwrap();
    std::fs::write(root.path().join("Note.md"), "# Second\n").unwrap();
    until(async || {
        row(&vault, "Note.md")
            .await
            .is_some_and(|value| value["content"] == "# Second\n")
    })
    .await;
    assert_eq!(
        row(&vault, "Diagram.canvas").await.unwrap()["content"],
        "pear"
    );
    close(runtime).await;
}

#[tokio::test]
async fn cold_reopen_recovers_a_deleted_canvas_projection_without_rewriting_source() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Diagram.canvas");
    let source = r#"{"nodes":[{"text":"cold source 📝"}]}"#;
    std::fs::write(&path, source).unwrap();
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    let id = Uuid::new_v4();
    let first = runtime(&profile);
    let vault = first.admit(id, root.path()).unwrap();
    vault
        .start_search_indexing_with(configuration(true, 256.0))
        .await
        .unwrap();
    until(async || row(&vault, "Diagram.canvas").await.is_some()).await;
    close(first).await;
    let database = lapis_app_core::database::Database::open(profile.path(), id)
        .await
        .unwrap();
    database
        .delete_search_document("Diagram.canvas")
        .await
        .unwrap();
    database.close().await.unwrap();
    let next = runtime(&profile);
    let vault = next.admit(id, root.path()).unwrap();
    vault
        .start_search_indexing_with(configuration(true, 512.0))
        .await
        .unwrap();
    until(async || row(&vault, "Diagram.canvas").await.is_some()).await;
    let recovered = row(&vault, "Diagram.canvas").await.unwrap();
    assert_eq!(recovered["content"], "cold source 📝");
    assert_eq!(
        recovered["sourceMetadata"]["chunking"]["targetChars"],
        512.0
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        modified
    );
    close(next).await;
}

#[cfg(unix)]
#[tokio::test]
async fn canvas_boundaries_preserve_foreign_rows_and_exclude_hidden_and_symlink_sources() {
    use lapis_app_protocol::database::search::records::SearchDocumentRecord;
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let source = r#"{"nodes":[{"text":"visible source"}]}"#;
    for path in ["Native.canvas", "Custom.canvas", ".hidden.canvas"] {
        std::fs::write(root.path().join(path), source).unwrap();
    }
    std::fs::write(outside.path().join("Secret.canvas"), source).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("Secret.canvas"),
        root.path().join("Escape.canvas"),
    )
    .unwrap();
    let id = Uuid::new_v4();
    let database = lapis_app_core::database::Database::open(profile.path(), id)
        .await
        .unwrap();
    for path in ["Custom.canvas", "Absent.canvas"] {
        let record: SearchDocumentRecord = serde_json::from_value(json!({
            "path":path,"sourceProviderId":"independent:canvas","name":path,
            "extension":"canvas","checksum":"retained","content":"foreign source",
            "tags":[],"tagParts":[],"tagHierarchy":[]
        }))
        .unwrap();
        database
            .upsert_prepared_search_document(record)
            .await
            .unwrap();
    }
    database.close().await.unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(id, root.path()).unwrap();
    vault
        .start_search_indexing_with(configuration(true, 256.0))
        .await
        .unwrap();
    until(async || row(&vault, "Native.canvas").await.is_some()).await;
    for path in ["Custom.canvas", "Absent.canvas"] {
        assert_eq!(row(&vault, path).await.unwrap()["checksum"], "retained");
    }
    assert!(row(&vault, ".hidden.canvas").await.is_none());
    assert!(row(&vault, "Escape.canvas").await.is_none());
    assert_eq!(
        std::fs::read_to_string(root.path().join("Custom.canvas")).unwrap(),
        source
    );
    close(runtime).await;
}
