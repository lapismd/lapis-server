use lapis_app_core::database::Database;
use lapis_app_protocol::ErrorCode;
use serde_json::json;
use std::{future::Future, path::Path, task::Poll};
use tokio::sync::broadcast::error::TryRecvError;
use uuid::Uuid;

async fn raw(profile: &Path, id: Uuid) -> (turso::Database, turso::Connection) {
    let engine = turso::Builder::new_local(profile.join(format!("{id}.db")).to_str().unwrap())
        .experimental_index_method(true)
        .build()
        .await
        .unwrap();
    let connection = engine.connect().unwrap();
    (engine, connection)
}

async fn strings(connection: &turso::Connection, sql: &str) -> Vec<String> {
    let mut rows = connection.query(sql, ()).await.unwrap();
    let mut values = vec![];
    while let Some(row) = rows.next().await.unwrap() {
        values.push(row.get::<String>(0).unwrap());
    }
    values
}

async fn version(connection: &turso::Connection) -> Vec<String> {
    strings(connection, "SELECT value FROM schema_meta WHERE key='schema.version' UNION ALL SELECT value_json FROM app_meta WHERE key='schema.version'").await
}

#[tokio::test]
async fn native_open_publishes_the_retained_turso_schema_version_without_a_change() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(db.get_meta("schema.version").await.unwrap(), Some(json!(5)));
    assert_eq!(db.revision().await.unwrap(), 0);
    let mut changes = db.subscribe_to_changes();
    db.migrate().await.unwrap();
    db.migrate().await.unwrap();
    assert_eq!(db.revision().await.unwrap(), 0);
    assert_eq!(changes.try_recv().unwrap_err(), TryRecvError::Empty);
    db.close().await.unwrap();
    assert_eq!(db.migrate().await.unwrap_err().code(), ErrorCode::Closed);
    let (_engine, connection) = raw(profile.path(), id).await;
    assert_eq!(version(&connection).await, ["5", "5"]);
}

#[tokio::test]
async fn explicit_migration_is_ordered_and_drains_after_the_waiter_detaches() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    db.set_meta("schema.version", Some(json!(42)))
        .await
        .unwrap();
    let mut changes = db.subscribe_to_changes();
    let mut migrate = Box::pin(db.migrate());
    std::future::poll_fn(|cx| {
        assert!(migrate.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(migrate);
    let mut following = Box::pin(db.set_meta("accepted-after-migration", Some(json!(true))));
    std::future::poll_fn(|cx| {
        assert!(following.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(following);
    db.close().await.unwrap();
    assert_eq!(changes.try_recv().unwrap().revision, 2);
    assert_eq!(changes.try_recv().unwrap_err(), TryRecvError::Closed);
    let (engine, connection) = raw(profile.path(), id).await;
    // Inspect before reopen, which itself performs idempotent migration.
    assert_eq!(version(&connection).await, ["5", "5"]);
    assert_eq!(
        strings(
            &connection,
            "SELECT value_json FROM app_meta WHERE key='accepted-after-migration'"
        )
        .await,
        ["true"]
    );
    drop(connection);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(db.revision().await.unwrap(), 2);
    db.close().await.unwrap();
}

#[tokio::test]
async fn current_retained_schema_keeps_all_durable_json_bytes_and_revisions() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let (engine, connection) = raw(profile.path(), id).await;
    // Use the serving provider's actual schema, not a duplicate test schema.
    let source = include_str!("../../../packages/api/src/lib/storage/turso-app-database.ts");
    assert!(source.contains("export const TURSO_APP_DATABASE_SCHEMA_VERSION = 5;"));
    let schema = source
        .split_once("export const TURSO_APP_DATABASE_SCHEMA = `")
        .unwrap()
        .1
        .split_once("`;")
        .unwrap()
        .0;
    connection.execute_batch(schema).await.unwrap();
    connection.execute_batch("INSERT INTO schema_meta VALUES('schema.version','5');
        INSERT INTO app_meta VALUES('schema.version','5');
        INSERT INTO app_meta VALUES('plugin:settings','{\"z\":1, \"a\":2}');
        INSERT INTO app_state VALUES(1,'frozen compatibility bytes');
        INSERT INTO app_revision VALUES(1,37);
        INSERT INTO app_changes VALUES(37,'[\"meta\"]','[]','null',1);
        INSERT INTO notifications VALUES('n','{\"z\":3, \"a\":4}');
        INSERT INTO history_files VALUES('f','{\"z\":5, \"a\":6}');
        INSERT INTO history_file_paths VALUES('Note.md','f');
        INSERT INTO history_revisions VALUES('f',0,'{\"z\":7, \"a\":8}');
        INSERT INTO task_records VALUES('Task.md','{\"z\":9, \"a\":10}');
        INSERT INTO memory_jobs VALUES('j','vault','completed',NULL,1,'{\"z\":11, \"a\":12}');
        INSERT INTO index_projection_rows VALUES('plugin/p','r','Note.md','note',0,'{\"z\":13, \"a\":14}');").await.unwrap();
    let payloads = "SELECT value_json FROM app_meta WHERE key='plugin:settings' \
        UNION ALL SELECT state_json FROM app_state \
        UNION ALL SELECT data_json FROM notifications \
        UNION ALL SELECT data_json FROM history_files \
        UNION ALL SELECT data_json FROM history_revisions \
        UNION ALL SELECT data_json FROM task_records \
        UNION ALL SELECT data_json FROM memory_jobs \
        UNION ALL SELECT data_json FROM index_projection_rows";
    let before = strings(&connection, payloads).await;
    drop(connection);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    let mut changes = db.subscribe_to_changes();
    db.migrate().await.unwrap();
    assert_eq!(db.revision().await.unwrap(), 37);
    assert_eq!(changes.try_recv().unwrap_err(), TryRecvError::Empty);
    db.close().await.unwrap();
    let (_engine, connection) = raw(profile.path(), id).await;
    assert_eq!(strings(&connection, payloads).await, before);
    assert_eq!(version(&connection).await, ["5", "5"]);
    assert_eq!(
        strings(&connection, "SELECT domains_json FROM app_changes").await,
        ["[\"meta\"]"]
    );
}

#[tokio::test]
async fn unversioned_native_initialization_is_additive_and_never_hydrates_app_state() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let (engine, connection) = raw(profile.path(), id).await;
    connection
        .execute_batch(
            "CREATE TABLE app_meta(key TEXT PRIMARY KEY,value_json TEXT NOT NULL);
        INSERT INTO app_meta VALUES('plugin:settings','{\"untouched\":true}');
        CREATE TABLE app_state(id INTEGER PRIMARY KEY,state_json TEXT NOT NULL);
        INSERT INTO app_state VALUES(1,'not normalized storage');
        CREATE TABLE app_revision(id INTEGER PRIMARY KEY,revision INTEGER NOT NULL);
        INSERT INTO app_revision VALUES(1,9);",
        )
        .await
        .unwrap();
    drop(connection);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(db.revision().await.unwrap(), 9);
    assert_eq!(
        db.get_meta("plugin:settings").await.unwrap(),
        Some(json!({"untouched":true}))
    );
    db.close().await.unwrap();
    let (_engine, connection) = raw(profile.path(), id).await;
    assert_eq!(version(&connection).await, ["5", "5"]);
    assert_eq!(
        strings(&connection, "SELECT state_json FROM app_state").await,
        ["not normalized storage"]
    );
}

#[tokio::test]
async fn rejected_schema_publication_rolls_back_new_tables_and_releases_the_owner() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let (engine, connection) = raw(profile.path(), id).await;
    connection.execute_batch("CREATE TABLE app_meta(key TEXT PRIMARY KEY CHECK(key!='schema.version'),value_json TEXT NOT NULL);
        INSERT INTO app_meta VALUES('keep','true');").await.unwrap();
    let schema_query = "SELECT sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name";
    let before = strings(&connection, schema_query).await;
    drop(connection);
    drop(engine);
    match Database::open(profile.path(), id).await {
        Err(error) => assert_eq!(error.code(), ErrorCode::Io),
        Ok(_) => panic!("Schema marker failure must prevent opening"),
    }
    let (engine, connection) = raw(profile.path(), id).await;
    assert_eq!(strings(&connection, schema_query).await, before);
    assert_eq!(
        strings(
            &connection,
            "SELECT value_json FROM app_meta WHERE key='keep'"
        )
        .await,
        ["true"]
    );
    connection
        .execute_batch(
            "ALTER TABLE app_meta RENAME TO saved_meta;
        CREATE TABLE app_meta(key TEXT PRIMARY KEY,value_json TEXT NOT NULL);
        INSERT INTO app_meta SELECT * FROM saved_meta; DROP TABLE saved_meta;",
        )
        .await
        .unwrap();
    drop(connection);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(db.get_meta("keep").await.unwrap(), Some(json!(true)));
    db.close().await.unwrap();
}

#[tokio::test]
async fn unsupported_versioned_formats_fail_without_rewriting_schema_or_payloads() {
    for value in ["4", "6", "invalid"] {
        let profile = tempfile::tempdir().unwrap();
        let id = Uuid::new_v4();
        let (engine, connection) = raw(profile.path(), id).await;
        connection
            .execute_batch(
                "CREATE TABLE schema_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
            CREATE TABLE app_meta(key TEXT PRIMARY KEY,value_json TEXT NOT NULL);
            INSERT INTO app_meta VALUES('keep','true');",
            )
            .await
            .unwrap();
        connection
            .execute(
                "INSERT INTO schema_meta VALUES('schema.version',?)",
                [value],
            )
            .await
            .unwrap();
        let schema_query = "SELECT sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name";
        let before = strings(&connection, schema_query).await;
        drop(connection);
        drop(engine);
        match Database::open(profile.path(), id).await {
            Err(error) => assert_eq!(error.code(), ErrorCode::Unsupported),
            Ok(_) => panic!("Unsupported version {value} must not open"),
        }
        let (_engine, connection) = raw(profile.path(), id).await;
        assert_eq!(strings(&connection, schema_query).await, before);
        assert_eq!(version(&connection).await, [value]);
        assert_eq!(
            strings(
                &connection,
                "SELECT value_json FROM app_meta WHERE key='keep'"
            )
            .await,
            ["true"]
        );
    }
}
