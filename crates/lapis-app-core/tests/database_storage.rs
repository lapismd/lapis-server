use lapis_app_core::database::{Database, MetaMutation, MetaTransaction};
use lapis_app_protocol::ErrorCode;
use serde_json::json;
use uuid::Uuid;

async fn raw(profile: &std::path::Path, id: Uuid) -> (turso::Database, turso::Connection) {
    let database = turso::Builder::new_local(profile.join(format!("{id}.db")).to_str().unwrap())
        .experimental_attach(false)
        .experimental_vacuum(false)
        .build()
        .await
        .unwrap();
    let connection = database.connect().unwrap();
    (database, connection)
}

#[tokio::test]
async fn metadata_facade_preserves_current_turso_schema_and_change_records() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let (engine, connection) = raw(profile.path(), id).await;
    // Read the actual serving schema, rather than a second test-only copy.
    let source = include_str!("../../../packages/api/src/lib/storage/turso-app-database.ts");
    let schema = source
        .split("export const TURSO_APP_DATABASE_SCHEMA = `")
        .nth(1)
        .unwrap()
        .split("`;")
        .next()
        .unwrap();
    connection.execute_batch(schema).await.unwrap();
    connection.execute_batch("INSERT INTO app_revision(id,revision) VALUES(1,8); INSERT INTO schema_meta(key,value) VALUES('schema.version','5'); INSERT INTO app_meta(key,value_json) VALUES('existing','{\"z\":1,\"a\":2}'); INSERT INTO files(path,normalized_path,extension,mtime,size,hash,indexed,deleted) VALUES('note.md','note.md','md',1,2,'hash',1,0);").await.unwrap();
    drop(connection);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(db.revision().await.unwrap(), 8);
    let expected = serde_json::from_str("{\"z\":1,\"a\":2}").unwrap();
    assert!(
        db.transact_meta(MetaTransaction {
            checks: vec![lapis_app_core::database::MetaCheck {
                key: "existing".into(),
                exists: Some(true),
                value: Some(expected)
            }],
            mutations: vec![MetaMutation {
                key: "new".into(),
                value: Some(json!("persisted")),
                delete: false
            }],
        })
        .await
        .unwrap()
    );
    assert!(
        !db.transact_meta(MetaTransaction {
            checks: vec![lapis_app_core::database::MetaCheck {
                key: "existing".into(),
                exists: None,
                value: Some(serde_json::from_str("{\"a\":2,\"z\":1}").unwrap())
            }],
            mutations: vec![MetaMutation {
                key: "new".into(),
                value: None,
                delete: true
            }],
        })
        .await
        .unwrap(),
        "Retain the existing JSON.stringify comparison order"
    );
    db.close().await.unwrap();
    let (_engine, connection) = raw(profile.path(), id).await;
    let mut rows = connection.query("SELECT s.value,f.hash,c.domains_json,c.paths_json,c.renamed_json FROM schema_meta s,files f,app_changes c WHERE s.key='schema.version' AND c.revision=9", ()).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    for (column, value) in [
        (0, "5"),
        (1, "hash"),
        (2, "[\"meta\"]"),
        (3, "[]"),
        (4, "null"),
    ] {
        assert_eq!(row.get::<String>(column).unwrap(), value);
    }
}

#[tokio::test]
async fn failed_native_transaction_rolls_back_all_mutations_and_revision_without_clearing_data() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let (engine, connection) = raw(profile.path(), id).await;
    connection.execute_batch("CREATE TABLE app_meta(key TEXT PRIMARY KEY CHECK(key!='reject'),value_json TEXT NOT NULL); INSERT INTO app_meta(key,value_json) VALUES('corrupt','{');").await.unwrap();
    drop(connection);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    let result = db
        .transact_meta(MetaTransaction {
            checks: vec![],
            mutations: vec![
                MetaMutation {
                    key: "first".into(),
                    value: Some(json!(true)),
                    delete: false,
                },
                MetaMutation {
                    key: "reject".into(),
                    value: Some(json!(true)),
                    delete: false,
                },
            ],
        })
        .await;
    assert_eq!(result.unwrap_err().code(), ErrorCode::Io);
    assert_eq!(db.get_meta("first").await.unwrap(), None);
    assert_eq!(db.revision().await.unwrap(), 0);
    assert_eq!(
        db.get_meta("corrupt").await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    db.set_meta("later", Some(json!("healthy"))).await.unwrap();
    assert_eq!(db.revision().await.unwrap(), 1);
    db.close().await.unwrap();
    let (_engine, connection) = raw(profile.path(), id).await;
    let mut rows = connection
        .query("SELECT value_json FROM app_meta WHERE key='corrupt'", ())
        .await
        .unwrap();
    assert_eq!(
        rows.next()
            .await
            .unwrap()
            .unwrap()
            .get::<String>(0)
            .unwrap(),
        "{"
    );
}
