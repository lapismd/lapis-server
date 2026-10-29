use lapis_app_core::database::Database;
use lapis_app_protocol::ErrorCode;
use serde_json::json;
use uuid::Uuid;
mod indexed_support;
mod projection_support;
use projection_support::{definition, source};

#[tokio::test]
async fn projection_row_reads_resolve_retained_id_path_and_document_aliases_without_writes() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    db.register_projection_definition(serde_json::from_value(definition("books/book")).unwrap())
        .await
        .unwrap();
    let mut input = source("books/book", "a.md", "row");
    input["rows"][0]["data"]["id"] = json!("public-id");
    input["rows"][0]["data"]["documentId"] = json!("document-id");
    let expected = input["rows"][0]["data"].clone();
    db.replace_projection_source(serde_json::from_value(input).unwrap())
        .await
        .unwrap();
    let revision = db.revision().await.unwrap();
    let mut changes = db.subscribe_to_changes();
    for alias in ["row", "a.md", "public-id", "document-id"] {
        assert_eq!(
            db.get_projection_row("books/book", alias, None)
                .await
                .unwrap(),
            Some(expected.clone())
        );
    }
    for missing in ["missing", "' OR 1=1 --"] {
        assert!(
            db.get_projection_row("books/book", missing, None)
                .await
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(db.revision().await.unwrap(), revision);
    assert!(matches!(
        changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    db.close().await.unwrap();
}

#[tokio::test]
async fn projection_row_reads_require_active_definitions_and_explicit_private_owner() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    assert!(
        db.get_projection_row("books/book", "row", None)
            .await
            .is_err()
    );
    let mut def = definition("books/book");
    def["visibility"] = json!("private");
    db.register_projection_definition(serde_json::from_value(def).unwrap())
        .await
        .unwrap();
    db.replace_projection_source(
        serde_json::from_value(source("books/book", "a.md", "row")).unwrap(),
    )
    .await
    .unwrap();
    for reader in [None, Some(""), Some("other")] {
        assert_eq!(
            db.get_projection_row("books/book", "row", reader)
                .await
                .unwrap_err()
                .0
                .code,
            ErrorCode::PermissionDenied
        );
    }
    assert!(
        db.get_projection_row("books/book", "row", Some("books"))
            .await
            .unwrap()
            .is_some()
    );
    db.unregister_projection_definition("books/book")
        .await
        .unwrap();
    assert!(
        db.get_projection_row("books/book", "row", Some("books"))
            .await
            .is_err()
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn projection_row_reads_hide_changed_file_hashes_and_old_definition_generations() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut def = definition("books/book");
    db.register_projection_definition(serde_json::from_value(def.clone()).unwrap())
        .await
        .unwrap();
    let file = indexed_support::record("a.md", 1);
    let mut input = source("books/book", "a.md", "row");
    input["sourceHash"] = json!(file.file.hash);
    db.upsert_indexed_metadata(file).await.unwrap();
    db.replace_projection_source(serde_json::from_value(input.clone()).unwrap())
        .await
        .unwrap();
    assert!(
        db.get_projection_row("books/book", "row", None)
            .await
            .unwrap()
            .is_some()
    );
    let mut file = indexed_support::record("a.md", 2);
    file.file.hash = "changed".into();
    db.upsert_indexed_metadata(file).await.unwrap();
    assert!(
        db.get_projection_row("books/book", "row", None)
            .await
            .unwrap()
            .is_none()
    );
    input["sourceHash"] = json!("changed");
    db.replace_projection_source(serde_json::from_value(input.clone()).unwrap())
        .await
        .unwrap();
    def["schemaVersion"] = json!(3);
    db.register_projection_definition(serde_json::from_value(def.clone()).unwrap())
        .await
        .unwrap();
    assert!(
        db.get_projection_row("books/book", "row", None)
            .await
            .unwrap()
            .is_none()
    );
    db.replace_projection_source(serde_json::from_value(input.clone()).unwrap())
        .await
        .unwrap();
    assert!(
        db.get_projection_row("books/book", "row", None)
            .await
            .unwrap()
            .is_some()
    );
    def["configHash"] = json!("new-config");
    db.register_projection_definition(serde_json::from_value(def).unwrap())
        .await
        .unwrap();
    assert!(
        db.get_projection_row("books/book", "row", None)
            .await
            .unwrap()
            .is_none()
    );
    db.replace_projection_source(serde_json::from_value(input).unwrap())
        .await
        .unwrap();
    let error = json!({"projectionId":"books/book","sourcePath":"a.md","sourceHash":"changed","error":"parser failed"});
    db.mark_projection_source_error(serde_json::from_value(error).unwrap())
        .await
        .unwrap();
    assert!(
        db.get_projection_row("books/book", "row", None)
            .await
            .unwrap()
            .is_none()
    );
    db.close().await.unwrap();
}
