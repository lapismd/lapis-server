use lapis_app_core::database::Database;
use lapis_app_protocol::ErrorCode;
use serde_json::{Value, json};
use uuid::Uuid;
mod indexed_support;
mod projection_support;
use projection_support::{definition, source};

async fn database() -> (tempfile::TempDir, Database) {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    db.register_projection_definition(serde_json::from_value(definition("books/book")).unwrap())
        .await
        .unwrap();
    for (id, rank) in [("a", 0), ("b", 1), ("c", 3)] {
        let mut input = source("books/book", &format!("{id}.md"), id);
        input["rows"][0]["data"] = json!({"id":id,"rank":rank});
        input["edges"] = if id == "a" {
            json!([
                {"sourceRowId":"a","relation":"child","targetRowId":"b","ordinal":0},
                {"sourceRowId":"a","relation":"child","targetRowId":"c","ordinal":1},
                {"sourceRowId":"a","relation":"path","targetPath":"b.md","ordinal":0}
            ])
        } else {
            json!([])
        };
        db.replace_projection_source(serde_json::from_value(input).unwrap())
            .await
            .unwrap();
    }
    (profile, db)
}
fn query(row: &str, relation: &str) -> Value {
    json!({"projectionId":"books/book","rowId":row,"relation":relation})
}

#[tokio::test]
async fn related_queries_filter_before_limits_resolve_incoming_paths_and_do_not_write() {
    let (_profile, db) = database().await;
    let revision = db.revision().await.unwrap();
    let mut changes = db.subscribe_to_changes();
    let mut outgoing = query("a", "child");
    outgoing["targetWhere"] = json!({"op":"compare","field":"rank","comparison":"gte","value":3});
    outgoing["limit"] = json!(1);
    let result = db
        .query_related(serde_json::from_value(outgoing).unwrap(), None)
        .await
        .unwrap();
    assert_eq!(result.rows, vec![json!({"id":"c","rank":3})]);
    let mut incoming = query("b", "path");
    incoming["direction"] = json!("in");
    assert_eq!(
        db.query_related(serde_json::from_value(incoming).unwrap(), None)
            .await
            .unwrap()
            .rows,
        vec![json!({"id":"a","rank":0})]
    );
    let mut incoming = query("c", "child");
    incoming["direction"] = json!("in");
    assert_eq!(
        db.query_related(serde_json::from_value(incoming).unwrap(), None)
            .await
            .unwrap()
            .rows,
        vec![json!({"id":"a","rank":0})]
    );
    assert_eq!(db.revision().await.unwrap(), revision);
    assert!(matches!(
        changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    db.close().await.unwrap();
}

#[tokio::test]
async fn related_queries_exclude_stale_targets_and_require_registered_private_target_owner() {
    let (_profile, db) = database().await;
    db.upsert_indexed_metadata(indexed_support::record("b.md", 2))
        .await
        .unwrap();
    assert!(
        db.query_related(serde_json::from_value(query("a", "path")).unwrap(), None)
            .await
            .unwrap()
            .rows
            .is_empty()
    );
    let mut private = definition("secret/row");
    private["visibility"] = json!("private");
    db.register_projection_definition(serde_json::from_value(private).unwrap())
        .await
        .unwrap();
    db.replace_projection_source(
        serde_json::from_value(source("secret/row", "secret.md", "secret")).unwrap(),
    )
    .await
    .unwrap();
    let mut input = source("books/book", "a.md", "a");
    input["edges"] = json!([{"sourceRowId":"a","relation":"private","targetProjectionId":"secret/row","targetRowId":"secret","ordinal":0},{"sourceRowId":"a","relation":"missing","targetProjectionId":"missing/row","targetRowId":"missing","ordinal":0}]);
    db.replace_projection_source(serde_json::from_value(input).unwrap())
        .await
        .unwrap();
    for reader in [None, Some("books"), Some("")] {
        assert_eq!(
            db.query_related(
                serde_json::from_value(query("a", "private")).unwrap(),
                reader
            )
            .await
            .unwrap_err()
            .0
            .code,
            ErrorCode::PermissionDenied
        );
    }
    assert_eq!(
        db.query_related(
            serde_json::from_value(query("a", "private")).unwrap(),
            Some("secret")
        )
        .await
        .unwrap()
        .rows
        .len(),
        1
    );
    assert_eq!(
        db.query_related(serde_json::from_value(query("a", "missing")).unwrap(), None)
            .await
            .unwrap_err()
            .0
            .code,
        ErrorCode::InvalidRequest
    );
    db.close().await.unwrap();
}
