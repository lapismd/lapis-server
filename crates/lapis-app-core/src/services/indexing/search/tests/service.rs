use super::support::*;
use lapis_app_protocol::database::search::{
    records::SearchDocumentRecord, retrieval::SearchOptions,
};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn native_markdown_selection_reconciles_search_without_any_client() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Note.md", "# Café 📝\n\nSource #team/notes\n");
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    until(async || vault.indexing_status().unwrap().passes > 0).await;
    let db = database(&vault).await;
    assert_eq!(db.get_search_index_stats().await.unwrap().document_count, 0);
    rescan(&vault).await;
    assert_eq!(db.get_search_index_stats().await.unwrap().document_count, 1);
    let row = db.get_search_document("Note.md").await.unwrap().unwrap();
    assert_eq!(row.source_provider_id.as_deref(), Some("search:markdown"));
    assert_eq!(row.tags, ["team/notes"]);
    assert_eq!(row.tag_hierarchy, ["team", "team/notes"]);
    assert_eq!(row.chunks.unwrap()[0].start_offset, 0.);
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
}

#[tokio::test]
async fn external_changes_reconcile_and_query_after_the_last_client_detaches() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Note.md", "# Before\n\nplum\n");
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    select(&vault, true).await;
    let db = database(&vault).await;
    until(async || db.get_search_index_stats().await.unwrap().document_count == 1).await;
    drop(client);
    write(&root, "Note.md", "# After\n\npeach 📝\n");
    write(&root, "New.markdown", "# Added\n\npeach\n");
    until(async || {
        db.search_document_paths("peach", SearchOptions::default())
            .await
            .unwrap()
            .len()
            == 2
    })
    .await;
    assert!(
        db.search_document_paths("plum", SearchOptions::default())
            .await
            .unwrap()
            .is_empty()
    );
    std::fs::remove_file(root.path().join("Note.md")).unwrap();
    until(async || db.get_search_document("Note.md").await.unwrap().is_none()).await;
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
}

#[tokio::test]
async fn unchanged_search_is_revision_stable_and_a_missing_cache_row_is_repaired() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Note.md", "# Stable\n");
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    select(&vault, true).await;
    let db = database(&vault).await;
    until(async || db.get_search_index_stats().await.unwrap().document_count == 1).await;
    let revision = db.revision().await.unwrap();
    let metadata = db.get_indexed_file("Note.md").await.unwrap().unwrap();
    rescan(&vault).await;
    assert_eq!(db.revision().await.unwrap(), revision);
    db.delete_search_document("Note.md").await.unwrap();
    rescan(&vault).await;
    assert!(db.get_search_document("Note.md").await.unwrap().is_some());
    assert_eq!(
        db.get_indexed_file("Note.md")
            .await
            .unwrap()
            .unwrap()
            .file
            .hash,
        metadata.file.hash
    );
    close(runtime).await;
}

#[tokio::test]
async fn foreign_providers_are_neither_overwritten_nor_pruned() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Custom.md", "# Actual file\n");
    write(&root, "Native.md", "# Native file\n");
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let db = database(&vault).await;
    for (path, provider) in [
        ("Custom.md", Some("custom:source")),
        ("Absent.md", Some("history:source")),
        ("Legacy.md", None),
    ] {
        let row: SearchDocumentRecord = serde_json::from_value(json!({
            "path":path, "sourceProviderId":provider, "name":path,
            "extension":"md", "checksum":"retained", "content":"Independent row",
            "tags":[], "tagParts":[], "tagHierarchy":[]
        }))
        .unwrap();
        db.upsert_prepared_search_document(row).await.unwrap();
    }
    select(&vault, true).await;
    until(async || db.get_search_document("Native.md").await.unwrap().is_some()).await;
    rescan(&vault).await;
    for path in ["Custom.md", "Absent.md", "Legacy.md"] {
        assert_eq!(
            db.get_search_document(path)
                .await
                .unwrap()
                .unwrap()
                .checksum,
            "retained"
        );
    }
    close(runtime).await;
}

#[tokio::test]
async fn owned_absent_rows_are_pruned_and_stale_provider_records_are_reprepared() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Note.md", "# Current\n");
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let db = database(&vault).await;
    for path in ["Note.md", "Absent.md"] {
        let row: SearchDocumentRecord = serde_json::from_value(json!({
            "path":path,"sourceProviderId":"search:markdown","name":path,
            "extension":"md","checksum":"stale","content":"Old source",
            "tags":[],"tagParts":[],"tagHierarchy":[]
        }))
        .unwrap();
        db.upsert_prepared_search_document(row).await.unwrap();
    }
    select(&vault, true).await;
    until(async || {
        db.get_search_document("Absent.md").await.unwrap().is_none()
            && db
                .get_search_document("Note.md")
                .await
                .unwrap()
                .is_some_and(|row| row.content == "# Current\n")
    })
    .await;
    let row = db.get_search_document("Note.md").await.unwrap().unwrap();
    assert_ne!(row.checksum, "stale");
    assert!(row.source_metadata.unwrap().projection_signature.is_some());
    close(runtime).await;
}

#[tokio::test]
async fn a_bad_source_reports_failure_while_a_healthy_peer_remains_queryable() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "a.md", "# Healthy\n");
    std::fs::write(root.path().join("z.md"), [0xff]).unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    select(&vault, true).await;
    let db = database(&vault).await;
    until(async || {
        vault.indexing_status().unwrap().error.is_some()
            && db.get_search_document("a.md").await.unwrap().is_some()
    })
    .await;
    assert!(db.get_search_document("z.md").await.unwrap().is_none());
    write(&root, "z.md", "# Repaired\n");
    until(async || {
        vault.indexing_status().unwrap().error.is_none()
            && db.get_search_document("z.md").await.unwrap().is_some()
    })
    .await;
    close(runtime).await;
}
