use super::support::*;
use lapis_app_protocol::database::projections::*;
use uuid::Uuid;

#[tokio::test]
async fn changed_root_prunes_bounded_orphan_error_pages_and_preserves_foreign_namespaces() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "docs/Old.doc.md", &note("old", "Old"));
    write(&root, "library/New.doc.md", &note("new", "New"));
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_docs_indexing("docs").await.unwrap();
    let database = database(&vault).await;
    until(async || row(&database, "docs/Old.doc.md").await["id"] == "old").await;
    vault.stop_docs_indexing().unwrap();
    rescan(&vault, "docs").await;
    vault.stop_docs_indexing().unwrap();
    let mut other = database
        .native_projection_state(ID, "")
        .await
        .unwrap()
        .definition
        .unwrap();
    other.projection_id = "other/note".into();
    other.owner_plugin_id = "other".into();
    database
        .register_projection_definition(other)
        .await
        .unwrap();
    database
        .mark_projection_source_error(ProjectionSourceError {
            projection_id: "other/note".into(),
            source_path: "foreign.doc.md".into(),
            source_hash: "foreign".into(),
            error: "foreign".into(),
            writer_plugin_id: Some("other".into()),
        })
        .await
        .unwrap();
    for index in 0..501 {
        database
            .mark_projection_source_error(ProjectionSourceError {
                projection_id: ID.into(),
                source_path: format!("former/{index:04}.doc.md"),
                source_hash: "orphan".into(),
                error: "orphan".into(),
                writer_plugin_id: Some("docs".into()),
            })
            .await
            .unwrap();
    }
    let first = database
        .native_projection_source_paths(ID, None)
        .await
        .unwrap();
    assert_eq!(first.len(), 500);
    let second = database
        .native_projection_source_paths(ID, first.last().map(String::as_str))
        .await
        .unwrap();
    assert_eq!(second.len(), 2);
    vault.start_docs_indexing("/library/").await.unwrap();
    until(async || row(&database, "library/New.doc.md").await["id"] == "new").await;
    until(async || {
        database
            .native_projection_source_paths(ID, None)
            .await
            .unwrap()
            == ["library/New.doc.md"]
    })
    .await;
    assert!(row(&database, "docs/Old.doc.md").await.is_null());
    assert_eq!(
        database
            .native_projection_state("other/note", "foreign.doc.md")
            .await
            .unwrap()
            .source
            .unwrap()
            .error
            .as_deref(),
        Some("foreign")
    );
    assert!(root.path().join("docs/Old.doc.md").exists());
    close(runtime).await;
}

#[cfg(unix)]
#[tokio::test]
async fn hidden_symlink_and_replaced_roots_never_admit_new_docs_bytes() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(&root, "docs/Visible.doc.md", &note("visible", "Visible"));
    write(
        &root,
        "docs/.hidden/Secret.doc.md",
        &note("secret", "Secret"),
    );
    write(&outside, "Outside.doc.md", &note("outside", "Outside"));
    std::os::unix::fs::symlink(
        outside.path().join("Outside.doc.md"),
        root.path().join("docs/Link.doc.md"),
    )
    .unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_docs_indexing("docs").await.unwrap();
    let database = database(&vault).await;
    until(async || row(&database, "docs/Visible.doc.md").await["id"] == "visible").await;
    assert_eq!(
        database
            .query_projection(ID, Default::default(), None)
            .await
            .unwrap()
            .rows
            .len(),
        1
    );
    for path in ["docs/.hidden/Secret.doc.md", "docs/Link.doc.md"] {
        database
            .mark_projection_source_error(ProjectionSourceError {
                projection_id: ID.into(),
                source_path: path.into(),
                source_hash: "old".into(),
                error: "old".into(),
                writer_plugin_id: Some("docs".into()),
            })
            .await
            .unwrap();
    }
    rescan(&vault, "docs").await;
    for path in ["docs/.hidden/Secret.doc.md", "docs/Link.doc.md"] {
        until(async || {
            database
                .native_projection_state(ID, path)
                .await
                .unwrap()
                .source
                .is_none()
        })
        .await;
    }
    let moved = outside.path().join("Admitted-root");
    std::fs::rename(root.path(), &moved).unwrap();
    std::fs::create_dir(root.path()).unwrap();
    write(
        &root,
        "docs/Replaced.doc.md",
        &note("replacement", "Replacement"),
    );
    assert_eq!(
        vault.start_docs_indexing("docs").await.unwrap_err().code(),
        lapis_app_protocol::ErrorCode::PermissionDenied
    );
    until(async || vault.indexing_status().unwrap().error.is_some()).await;
    assert!(row(&database, "docs/Replaced.doc.md").await.is_null());
    close(runtime).await;
}

#[tokio::test]
async fn missing_or_stale_core_metadata_keeps_docs_publication_pending() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "docs/Note.doc.md", &note("note", "Note"));
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let database = database(&vault).await;
    let admitted = crate::vault::VaultRoot::open(root.path()).unwrap();
    let definition = super::super::definition::definition("docs").unwrap();
    assert_eq!(
        super::super::source::publish(
            &database,
            &admitted,
            &definition,
            "docs/Note.doc.md",
            || false
        )
        .await
        .unwrap(),
        1
    );
    let path = "docs/Note.doc.md";
    let raw = note("note", "Note");
    let stat = admitted.stat(path).unwrap().unwrap();
    let mut record = crate::services::indexing::records::record(
        path,
        &stat,
        &raw,
        &[path.into()],
        &Default::default(),
    );
    record.metadata.parser_version = "stale-parser".into();
    database.upsert_indexed_metadata(record).await.unwrap();
    assert_eq!(
        super::super::source::publish(&database, &admitted, &definition, path, || false)
            .await
            .unwrap(),
        1
    );
    assert!(
        database
            .native_projection_state(ID, "docs/Note.doc.md")
            .await
            .unwrap()
            .source
            .is_none()
    );
    close(runtime).await;
}
