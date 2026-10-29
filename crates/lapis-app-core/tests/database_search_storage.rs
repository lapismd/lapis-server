use lapis_app_core::database::Database;
use lapis_app_protocol::ErrorCode;
use serde_json::json;
use uuid::Uuid;
mod search_support;
use search_support::{assert_json, fixture, seed};
mod projection_support;

async fn raw(dir: &std::path::Path, id: Uuid) -> (turso::Database, turso::Connection) {
    let engine = turso::Builder::new_local(dir.join(format!("{id}.db")).to_str().unwrap())
        .experimental_index_method(true)
        .build()
        .await
        .unwrap();
    let connection = engine.connect().unwrap();
    (engine, connection)
}
async fn checkpoint(connection: &turso::Connection) {
    let mut rows = connection
        .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
        .await
        .unwrap();
    assert_eq!(
        rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
        0
    );
}
#[tokio::test]
async fn existing_turso_search_schema_and_fts_survive_native_publication_and_float_vectors() {
    let dir = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let (engine, connection) = raw(dir.path(), id).await;
    let source = include_str!("../../../packages/api/src/lib/storage/turso-app-database.ts");
    for marker in [
        "export const TURSO_APP_DATABASE_SCHEMA = `",
        "const TURSO_SEARCH_FTS_SCHEMA = `",
    ] {
        let schema = source
            .split(marker)
            .nth(1)
            .unwrap()
            .split("`;")
            .next()
            .unwrap();
        connection.execute_batch(schema).await.unwrap();
    }
    connection.execute_batch("INSERT INTO app_revision(id,revision) VALUES(1,8); INSERT INTO schema_meta(key,value) VALUES('schema.version','5'); INSERT INTO app_meta(key,value_json) VALUES('unrelated','true');").await.unwrap();
    checkpoint(&connection).await;
    drop(connection);
    drop(engine);
    let db = Database::open(dir.path(), id).await.unwrap();
    seed(&db).await;
    assert_eq!(db.revision().await.unwrap(), 12);
    assert_eq!(db.get_meta("unrelated").await.unwrap(), Some(json!(true)));
    let mut events = db.subscribe_to_changes();
    let mut rejected = fixture()["records"][0]["stored"].clone();
    rejected["content"] = json!("mustnotpersist");
    rejected["chunks"] =
        json!([{"id":"B.md#chunk-17-26","text":"collision","startOffset":0,"endOffset":9}]);
    assert_eq!(
        db.upsert_prepared_search_document(serde_json::from_value(rejected).unwrap())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Io
    );
    assert_eq!(db.revision().await.unwrap(), 12);
    assert!(events.try_recv().is_err());
    assert_json(
        serde_json::to_value(db.get_search_document("A.md").await.unwrap().unwrap()).unwrap(),
        fixture()["records"][0]["stored"].clone(),
    );
    db.close().await.unwrap();
    let (engine, connection) = raw(dir.path(), id).await;
    let mut vectors=connection.query("SELECT typeof(embedding),vector_distance_cos(embedding,vector32('[1,0,0]')) FROM search_chunks WHERE id='A.md#ready'",()).await.unwrap();
    let row = vectors.next().await.unwrap().unwrap();
    assert_eq!(row.get::<String>(0).unwrap(), "blob");
    assert!(matches!(row.get_value(1).unwrap(),turso::Value::Real(v) if v.abs()<0.000001));
    drop(vectors);
    let mut fts=connection.query("SELECT path FROM search_docs WHERE fts_match(name,path,content,tags,metadata_text,'alpha')",()).await.unwrap();
    assert_eq!(
        fts.next().await.unwrap().unwrap().get::<String>(0).unwrap(),
        "A.md"
    );
    drop(fts);
    let mut absent=connection.query("SELECT path FROM search_docs WHERE fts_match(name,path,content,tags,metadata_text,'mustnotpersist')",()).await.unwrap();
    assert!(absent.next().await.unwrap().is_none());
    drop(absent);
    let mut version = connection
        .query(
            "SELECT value FROM schema_meta WHERE key='schema.version'",
            (),
        )
        .await
        .unwrap();
    assert_eq!(
        version
            .next()
            .await
            .unwrap()
            .unwrap()
            .get::<String>(0)
            .unwrap(),
        "5"
    );
    drop(version);
    drop(connection);
    drop(engine);
}
#[tokio::test]
async fn search_manifest_and_stats_never_hydrate_bodies_and_unknown_embeddings_remain_pending() {
    let dir = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(dir.path(), id).await.unwrap();
    seed(&db).await;
    db.close().await.unwrap();
    let (engine, connection) = raw(dir.path(), id).await;
    connection.execute_batch("UPDATE search_docs SET data_json='not document JSON'; UPDATE search_chunks SET embedding_json='{\"status\":\"future-state\"}' WHERE id='A.md#pending'").await.unwrap();
    checkpoint(&connection).await;
    drop(connection);
    drop(engine);
    let db = Database::open(dir.path(), id).await.unwrap();
    assert_json(
        serde_json::to_value(
            db.list_search_document_manifest(Default::default())
                .await
                .unwrap()
                .rows,
        )
        .unwrap(),
        fixture()["manifest"].clone(),
    );
    assert_json(
        serde_json::to_value(db.get_search_index_stats().await.unwrap()).unwrap(),
        fixture()["stats"].clone(),
    );
    assert_eq!(
        db.get_search_document("A.md").await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    db.close().await.unwrap();
}
#[tokio::test]
async fn failed_search_schema_intake_rolls_back_new_tables_and_preserves_existing_data() {
    let dir = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(dir.path(), id).await.unwrap();
    db.set_meta("retained", Some(json!("still here")))
        .await
        .unwrap();
    db.close().await.unwrap();
    let (engine, connection) = raw(dir.path(), id).await;
    connection.execute_batch("DROP TABLE search_chunks; DROP TABLE search_docs; CREATE TABLE search_chunks(id TEXT PRIMARY KEY)").await.unwrap();
    checkpoint(&connection).await;
    drop(connection);
    drop(engine);
    assert_eq!(
        Database::open(dir.path(), id).await.err().unwrap().code(),
        ErrorCode::Io
    );
    assert!(
        projection_support::inspect(
            dir.path(),
            id,
            "SELECT name FROM sqlite_schema WHERE name='search_docs'"
        )
        .await
        .is_empty()
    );
    assert_eq!(
        projection_support::inspect(
            dir.path(),
            id,
            "SELECT value_json FROM app_meta WHERE key='retained'"
        )
        .await,
        json!([["\"still here\""]])
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_array().unwrap().clone())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn search_message_bounds_reject_large_materialization_without_breaking_manifests_or_storage()
{
    use lapis_app_protocol::database::search::{
        SearchManifestQuery, records::SearchDocumentRecord,
    };
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path(), Uuid::new_v4()).await.unwrap();
    let mut record: SearchDocumentRecord =
        serde_json::from_value(fixture()["records"][3]["stored"].clone()).unwrap();
    record.content = "x".repeat(4 * 1024 * 1024);
    for i in 0..9 {
        record.path = format!("bounded-{i}.md");
        db.upsert_prepared_search_document(record.clone())
            .await
            .unwrap();
    }
    let before = db.revision().await.unwrap();
    assert_eq!(
        db.list_search_documents().await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        db.list_search_document_manifest(Default::default())
            .await
            .unwrap()
            .rows
            .len(),
        9
    );
    assert_eq!(db.get_search_index_stats().await.unwrap().document_count, 9);
    assert_eq!(
        db.get_search_document("bounded-0.md")
            .await
            .unwrap()
            .unwrap()
            .content
            .len(),
        4 * 1024 * 1024
    );
    assert_eq!(
        db.list_search_document_manifest(SearchManifestQuery {
            limit: Some(usize::MAX),
            after: None
        })
        .await
        .unwrap_err()
        .code(),
        ErrorCode::InvalidRequest
    );
    record.content = "x".repeat(32 * 1024 * 1024);
    assert_eq!(
        db.upsert_prepared_search_document(record)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(db.revision().await.unwrap(), before);
    db.close().await.unwrap();
}
