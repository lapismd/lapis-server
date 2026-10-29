use lapis_app_core::database::Database;
use lapis_app_protocol::ErrorCode;
use serde_json::{Value, json};
use uuid::Uuid;
mod indexed_support;
mod projection_support;
use projection_support::definition;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/projection-query.json"
    ))
    .unwrap()
}

#[tokio::test]
async fn selection_and_page_limits_bound_large_corpora_and_preserve_own_json_fields() {
    let (_profile, db) = database().await;
    let payload = "x".repeat(3 * 1024 * 1024);
    for n in 0..14 {
        let id = format!("large-{n}");
        db.replace_projection_source(serde_json::from_value(json!({"projectionId":"books/book","sourcePath":format!("{id}.md"),"sourceHash":"h","rows":[{"id":id,"kind":"large","data":{"id":id,"large":true,"rank":20-n,"payload":payload,"__proto__":{"own":true},"constructor":"value"}}]})).unwrap()).await.unwrap();
    }
    let revision = db.revision().await.unwrap();
    let mut query = json!({"where":{"op":"compare","field":"large","comparison":"eq","value":true},"orderBy":[{"field":"rank","direction":"asc"}],"limit":2,"select":["id","__proto__","constructor","toString"]});
    let result = db
        .query_projection(
            "books/book",
            serde_json::from_value(query.clone()).unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        result.rows,
        vec![
            json!({"id":"large-13","__proto__":{"own":true},"constructor":"value"}),
            json!({"id":"large-12","__proto__":{"own":true},"constructor":"value"})
        ]
    );
    assert!(result.next_cursor.is_some());
    query.as_object_mut().unwrap().remove("select");
    query.as_object_mut().unwrap().remove("limit");
    assert_eq!(
        db.query_projection("books/book", serde_json::from_value(query).unwrap(), None)
            .await
            .unwrap_err()
            .0
            .code,
        ErrorCode::InvalidRequest
    );
    assert_eq!(db.revision().await.unwrap(), revision);
    db.close().await.unwrap();
}
async fn database() -> (tempfile::TempDir, Database) {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut def = definition("books/book");
    def["fields"] =
        json!({"title":{"type":"string","sortable":true},"rank":{"type":"number","sortable":true}});
    db.register_projection_definition(serde_json::from_value(def).unwrap())
        .await
        .unwrap();
    for row in fixture()["rows"].as_array().unwrap() {
        db.replace_projection_source(serde_json::from_value(json!({
            "projectionId":"books/book", "sourcePath":format!("{}.md",row["id"].as_str().unwrap()),
            "sourceHash":"fixture-hash", "rows":[{"id":row["id"],"kind":"book","data":row}]
        })).unwrap()).await.unwrap();
    }
    (profile, db)
}

#[tokio::test]
async fn native_query_ast_matches_every_retained_browser_fixture_without_writes() {
    let (_profile, db) = database().await;
    let revision = db.revision().await.unwrap();
    let mut changes = db.subscribe_to_changes();
    for case in fixture()["cases"].as_array().unwrap() {
        let result = db
            .query_projection(
                "books/book",
                serde_json::from_value(case["query"].clone()).unwrap(),
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            result
                .rows
                .iter()
                .map(|r| r["id"].clone())
                .collect::<Vec<_>>(),
            *case["ids"].as_array().unwrap(),
            "{}",
            case["name"]
        );
        assert_eq!(result.revision, revision);
        assert!(result.complete);
        assert_eq!(
            serde_json::to_value(result).unwrap()["indexStatus"],
            "ready"
        );
    }
    assert_eq!(db.revision().await.unwrap(), revision);
    assert!(matches!(
        changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    // Retain JavaScript's positive slice limit and non-positive unlimited default.
    for (limit, count) in [(0.0, 5), (-1.0, 5), (0.5, 0), (1.9, 1)] {
        assert_eq!(
            db.query_projection(
                "books/book",
                serde_json::from_value(json!({"limit":limit})).unwrap(),
                None
            )
            .await
            .unwrap()
            .rows
            .len(),
            count
        );
    }
    db.close().await.unwrap();
}

#[tokio::test]
async fn native_query_pages_retain_null_placement_locale_ties_and_browser_cursors() {
    let (_profile, db) = database().await;
    let mut query = json!({"orderBy":[{"field":"rank","direction":"asc","nulls":"last"}],"limit":2,"select":["id","rank"]});
    let first = db
        .query_projection(
            "books/book",
            serde_json::from_value(query.clone()).unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        first.rows,
        vec![json!({"id":"b","rank":1}), json!({"id":"a","rank":2})]
    );
    query["after"] = json!(first.next_cursor.unwrap());
    let second = db
        .query_projection(
            "books/book",
            serde_json::from_value(query.clone()).unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        second.rows,
        vec![json!({"id":"c","rank":3}), json!({"id":"e","rank":4})]
    );
    query["after"] = json!(second.next_cursor.unwrap());
    let third = db
        .query_projection("books/book", serde_json::from_value(query).unwrap(), None)
        .await
        .unwrap();
    assert_eq!(third.rows, vec![json!({"id":"d","rank":null})]);
    assert!(third.next_cursor.is_none());
    // btoa(JSON.stringify({values:[null],rowId:"d"})) from the retained client.
    let query = json!({"orderBy":[{"field":"rank","direction":"asc","nulls":"first"}],"limit":1,"after":"eyJ2YWx1ZXMiOltudWxsXSwicm93SWQiOiJkIn0="});
    assert_eq!(
        db.query_projection("books/book", serde_json::from_value(query).unwrap(), None)
            .await
            .unwrap()
            .rows[0]["id"],
        "b"
    );
    for id in ["tie-a", "tie-B", "tie-c"] {
        db.replace_projection_source(serde_json::from_value(json!({"projectionId":"books/book","sourcePath":format!("{id}.md"),"sourceHash":"h","rows":[{"id":id,"kind":"tie","data":{"id":id,"ties":true}}]})).unwrap()).await.unwrap();
    }
    let mut query =
        json!({"where":{"op":"compare","field":"ties","comparison":"eq","value":true},"limit":1});
    let first = db
        .query_projection(
            "books/book",
            serde_json::from_value(query.clone()).unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(first.rows[0]["id"], "tie-a");
    query["after"] = json!(first.next_cursor.unwrap());
    assert_eq!(
        db.query_projection("books/book", serde_json::from_value(query).unwrap(), None)
            .await
            .unwrap()
            .rows[0]["id"],
        "tie-B"
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn native_queries_retain_source_status_privacy_and_invalid_cursor_errors() {
    let (_profile, db) = database().await;
    for cursor in ["not-base64", "e30=", "bnVsbA=="] {
        assert_eq!(
            db.query_projection(
                "books/book",
                serde_json::from_value(json!({"after":cursor})).unwrap(),
                None
            )
            .await
            .unwrap_err()
            .0
            .code,
            ErrorCode::InvalidRequest
        );
    }
    db.mark_projection_source_error(serde_json::from_value(json!({"projectionId":"books/book","sourcePath":"a.md","sourceHash":"fixture-hash","error":"failed"})).unwrap()).await.unwrap();
    // Error publication removes a's rows. A separate changed hash makes b stale.
    db.upsert_indexed_metadata(indexed_support::record("b.md", 1))
        .await
        .unwrap();
    let result = db
        .query_projection(
            "books/book",
            serde_json::from_value(json!({})).unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 3);
    assert_eq!(
        serde_json::to_value(result).unwrap()["indexStatus"],
        "error"
    );
    let result = db
        .query_projection(
            "books/book",
            serde_json::from_value(json!({"includeStale":true})).unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 4);
    let mut def = definition("books/book");
    def["visibility"] = json!("private");
    db.register_projection_definition(serde_json::from_value(def).unwrap())
        .await
        .unwrap();
    assert_eq!(
        db.query_projection(
            "books/book",
            serde_json::from_value(json!({})).unwrap(),
            None
        )
        .await
        .unwrap_err()
        .0
        .code,
        ErrorCode::PermissionDenied
    );
    assert!(
        db.query_projection(
            "books/book",
            serde_json::from_value(json!({})).unwrap(),
            Some("books")
        )
        .await
        .is_ok()
    );
    db.close().await.unwrap();
}
