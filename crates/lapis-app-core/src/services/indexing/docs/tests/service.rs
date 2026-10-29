use super::support::*;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn selection_reuses_owner_and_publishes_registered_note_with_core_metadata() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let raw = "---\nid: alpha\ntitle: Alpha\ndocType: freeform\ntags: [one, one, 2]\ncards: [null, {}]\nbookId: book-1\n---\n# Body\n";
    write(&root, "docs/Alpha.doc.md", raw);
    write(&root, "docs/Nested/Beta.DOC.MD", &note("beta", "Beta"));
    write(&root, "docs-other/Other.doc.md", &note("other", "Other"));
    if fs_distinguishes_case(&root) {
        // Sibling-case directory exclusion is unrepresentable on
        // case-insensitive filesystems; those hosts skip only this seed.
        write(&root, "Docs/Case.doc.md", &note("case", "Case"));
    }
    write(&root, "docs/Plain.md", &note("plain", "Plain"));
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    until(async || vault.indexing_status().unwrap().passes > 0).await;
    let database = database(&vault).await;
    assert!(
        database
            .query_projection(ID, Default::default(), None)
            .await
            .is_err()
    );
    let owner = vault
        .indexing
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .status
        .clone();
    vault.start_docs_indexing(" /docs/ ").await.unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &owner,
        &vault.indexing.lock().unwrap().as_ref().unwrap().status
    ));
    until(async || row(&database, "docs/Alpha.doc.md").await["id"] == "alpha").await;
    assert_eq!(
        row(&database, "docs/Alpha.doc.md").await,
        json!({
            "id":"alpha", "title":"Alpha", "docType":"freeform", "path":"docs/Alpha.doc.md",
            "tags":["one","2"], "cardCount":2, "bookId":"book-1"
        })
    );
    until(async || row(&database, "docs/Nested/Beta.DOC.MD").await["id"] == "beta").await;
    assert_eq!(
        database
            .query_projection(ID, Default::default(), None)
            .await
            .unwrap()
            .rows
            .len(),
        2
    );
    let state = database
        .native_projection_state(ID, "docs/Alpha.doc.md")
        .await
        .unwrap();
    let definition = state.definition.unwrap();
    assert_eq!(definition.schema_version, 1.0);
    assert_eq!(definition.owner_plugin_id, "docs");
    assert_eq!(definition.fields.len(), 7);
    assert_eq!(state.source.unwrap().status, "ready");
    let indexed = database
        .get_indexed_file("docs/Alpha.doc.md")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(indexed.file.extension, "md");
    assert_eq!(
        indexed.file.hash,
        lapis_plugin_services::markdown::content_hash(raw)
    );
    assert_eq!(indexed.file.size, raw.len() as f64);
    assert_eq!(
        indexed.metadata.unwrap().metadata["headings"][0]["heading"],
        "Body"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("docs/Alpha.doc.md")).unwrap(),
        raw
    );
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
}

#[tokio::test]
async fn parser_errors_remove_ready_rows_remain_idempotent_and_recover_independently() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "docs/Bad.doc.md", &note("Bad", "Invalid"));
    write(&root, "docs/Good.doc.md", &note("good", "Good"));
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_docs_indexing("docs").await.unwrap();
    let database = database(&vault).await;
    until(async || {
        database
            .native_projection_state(ID, "docs/Bad.doc.md")
            .await
            .unwrap()
            .source
            .is_some_and(|source| source.status == "error")
    })
    .await;
    until(async || row(&database, "docs/Good.doc.md").await["id"] == "good").await;
    let source = database
        .native_projection_state(ID, "docs/Bad.doc.md")
        .await
        .unwrap()
        .source
        .unwrap();
    assert_eq!(source.error.as_deref(), Some("Invalid docs id: Bad"));
    assert!(row(&database, "docs/Bad.doc.md").await.is_null());
    let revision = database.revision().await.unwrap();
    rescan(&vault, "docs").await;
    assert_eq!(database.revision().await.unwrap(), revision);
    write(&root, "docs/Good.doc.md", "---\ncards: [{id: Bad}]\n---\n");
    until(async || {
        database
            .native_projection_state(ID, "docs/Good.doc.md")
            .await
            .unwrap()
            .source
            .is_some_and(|source| source.error.as_deref() == Some("Invalid docs card id: Bad"))
    })
    .await;
    assert!(row(&database, "docs/Good.doc.md").await.is_null());
    write(&root, "docs/Bad.doc.md", &note("recovered", "Recovered"));
    until(async || row(&database, "docs/Bad.doc.md").await["id"] == "recovered").await;
    write(&root, "docs/Bad.doc.md", "---\ntitle: [broken\n---\n");
    until(async || {
        database
            .native_projection_state(ID, "docs/Bad.doc.md")
            .await
            .unwrap()
            .source
            .is_some_and(|source| {
                source.status == "error"
                    && source
                        .error
                        .is_some_and(|error| error.contains("flow sequence"))
            })
    })
    .await;
    close(runtime).await;
}

#[tokio::test]
async fn missing_stale_or_damaged_projection_sources_repair_without_metadata_changes() {
    use lapis_app_protocol::database::projections::*;
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "docs/Note.doc.md", &note("note", "Note"));
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_docs_indexing("docs").await.unwrap();
    let database = database(&vault).await;
    until(async || row(&database, "docs/Note.doc.md").await["id"] == "note").await;
    let original =
        serde_json::to_value(database.get_indexed_file("docs/Note.doc.md").await.unwrap()).unwrap();
    database
        .delete_projection_source(ID, "docs/Note.doc.md", Some("docs"))
        .await
        .unwrap();
    rescan(&vault, "docs").await;
    until(async || row(&database, "docs/Note.doc.md").await["id"] == "note").await;
    let state = database
        .native_projection_state(ID, "docs/Note.doc.md")
        .await
        .unwrap()
        .source
        .unwrap();
    database
        .replace_projection_source(ProjectionSource {
            projection_id: ID.into(),
            source_path: "docs/Note.doc.md".into(),
            source_hash: state.hash.clone(),
            rows: vec![ProjectionRow {
                id: "note".into(),
                kind: "note".into(),
                ordinal: None,
                data: json!({"id":"note","title":"Damaged"})
                    .as_object()
                    .unwrap()
                    .clone(),
            }],
            edges: vec![],
            writer_plugin_id: Some("docs".into()),
        })
        .await
        .unwrap();
    rescan(&vault, "docs").await;
    until(async || row(&database, "docs/Note.doc.md").await["title"] == "Note").await;
    database
        .mark_projection_source_error(ProjectionSourceError {
            projection_id: ID.into(),
            source_path: "docs/Note.doc.md".into(),
            source_hash: state.hash,
            error: "stale".into(),
            writer_plugin_id: Some("docs".into()),
        })
        .await
        .unwrap();
    rescan(&vault, "docs").await;
    until(async || row(&database, "docs/Note.doc.md").await["id"] == "note").await;
    assert_eq!(
        serde_json::to_value(database.get_indexed_file("docs/Note.doc.md").await.unwrap()).unwrap(),
        original
    );
    close(runtime).await;
}
