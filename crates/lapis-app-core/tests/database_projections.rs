use lapis_app_core::database::Database;
use serde_json::{Value, json};
use uuid::Uuid;

mod indexed_support;
mod projection_support;
use projection_support::{definition, inspect, source};

#[tokio::test]
async fn projection_source_persists_rows_edges_and_typed_direct_property_values() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    db.register_projection_definition(serde_json::from_value(definition("books/book")).unwrap())
        .await
        .unwrap();
    let mut changes = db.subscribe_to_changes();
    db.replace_projection_source(
        serde_json::from_value(source("books/book", "a.md", "a")).unwrap(),
    )
    .await
    .unwrap();
    let change = changes.recv().await.unwrap();
    assert_eq!(change.revision, 2);
    assert_eq!(
        serde_json::to_value(change.domains).unwrap(),
        json!(["projection"])
    );
    assert_eq!(change.paths, ["a.md"]);
    db.close().await.unwrap();
    let values = inspect(profile.path(), id, "SELECT field,ordinal,text_value,number_value,boolean_value,date_value FROM index_projection_values ORDER BY field,ordinal").await;
    assert_eq!(
        values,
        vec![
            vec![
                json!("done"),
                json!(0),
                Value::Null,
                Value::Null,
                json!(1),
                Value::Null
            ],
            vec![
                json!("literal.key"),
                json!(0),
                Value::Null,
                Value::Null,
                Value::Null,
                json!("2026-10-24")
            ],
            vec![
                json!("score"),
                json!(0),
                Value::Null,
                json!(42.0),
                Value::Null,
                Value::Null
            ],
            vec![
                json!("tags"),
                json!(0),
                json!("one"),
                Value::Null,
                Value::Null,
                Value::Null
            ],
            vec![
                json!("tags"),
                json!(1),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null
            ],
            vec![
                json!("tags"),
                json!(2),
                json!("two"),
                Value::Null,
                Value::Null,
                Value::Null
            ],
            vec![
                json!("title"),
                json!(0),
                json!("native 🪨"),
                Value::Null,
                Value::Null,
                Value::Null
            ]
        ]
    );
    assert_eq!(inspect(profile.path(), id, "SELECT source_hash,schema_version,config_hash,status,error FROM index_projection_sources").await, vec![vec![json!("hash-1"),json!(2),json!("config-2"),json!("ready"),Value::Null]]);
    assert_eq!(
        inspect(
            profile.path(),
            id,
            "SELECT source_row_id,relation,target_path,data_json FROM index_projection_edges"
        )
        .await,
        vec![vec![
            json!("a"),
            json!("reference"),
            json!("target.md"),
            json!("{\"label\":\"link\"}")
        ]]
    );
}

#[tokio::test]
async fn projection_writes_enforce_owner_active_definition_and_row_bounds() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    let mut input = source("books/book", "a.md", "a");
    assert!(
        db.replace_projection_source(serde_json::from_value(input.clone()).unwrap())
            .await
            .is_err()
    );
    db.register_projection_definition(serde_json::from_value(definition("books/book")).unwrap())
        .await
        .unwrap();
    input["writerPluginId"] = json!("foreign");
    assert!(
        db.replace_projection_source(serde_json::from_value(input.clone()).unwrap())
            .await
            .is_err()
    );
    assert!(
        db.delete_projection_source("books/book", "a.md", Some("foreign"))
            .await
            .is_err()
    );
    let error = json!({"projectionId":"books/book","sourcePath":"a.md","sourceHash":"hash-1","error":"failed","writerPluginId":"foreign"});
    assert!(
        db.mark_projection_source_error(serde_json::from_value(error).unwrap())
            .await
            .is_err()
    );
    input["writerPluginId"] = json!("books");
    input["rows"] = Value::Array(vec![input["rows"][0].clone(); 65]);
    assert!(
        db.replace_projection_source(serde_json::from_value(input).unwrap())
            .await
            .is_err()
    );
    assert_eq!(db.revision().await.unwrap(), 1);
    db.unregister_projection_definition("books/book")
        .await
        .unwrap();
    assert!(
        db.replace_projection_source(
            serde_json::from_value(source("books/book", "a.md", "a")).unwrap()
        )
        .await
        .is_err()
    );
    assert_eq!(db.revision().await.unwrap(), 2);
    db.close().await.unwrap();
}

#[tokio::test]
async fn failed_projection_replacement_rolls_back_every_domain_without_publishing() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    db.upsert_indexed_metadata(indexed_support::record("a.md", 1))
        .await
        .unwrap();
    db.register_projection_definition(serde_json::from_value(definition("books/book")).unwrap())
        .await
        .unwrap();
    db.replace_projection_source(
        serde_json::from_value(source("books/book", "a.md", "a")).unwrap(),
    )
    .await
    .unwrap();
    db.replace_projection_source(
        serde_json::from_value(source("books/book", "b.md", "b")).unwrap(),
    )
    .await
    .unwrap();
    let mut changes = db.subscribe_to_changes();
    let mut input = source("books/book", "a.md", "a");
    input["rows"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"b","kind":"duplicate","data":{}}));
    assert!(
        db.replace_projection_source(serde_json::from_value(input).unwrap())
            .await
            .is_err()
    );
    assert_eq!(db.revision().await.unwrap(), 4);
    assert!(db.get_indexed_file("a.md").await.unwrap().is_some());
    assert!(matches!(
        changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    db.close().await.unwrap();
    assert_eq!(
        inspect(
            profile.path(),
            id,
            "SELECT row_id,source_path FROM index_projection_rows ORDER BY row_id"
        )
        .await,
        vec![
            vec![json!("a"), json!("a.md")],
            vec![json!("b"), json!("b.md")]
        ]
    );
    assert_eq!(
        inspect(
            profile.path(),
            id,
            "SELECT COUNT(*) FROM index_projection_values"
        )
        .await[0][0],
        json!(14)
    );
    assert_eq!(
        inspect(
            profile.path(),
            id,
            "SELECT COUNT(*) FROM index_projection_edges"
        )
        .await[0][0],
        json!(2)
    );
}

#[tokio::test]
async fn source_error_delete_and_unregister_clean_only_the_owned_projection_data() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    for projection in ["books/book", "other/book"] {
        db.register_projection_definition(serde_json::from_value(definition(projection)).unwrap())
            .await
            .unwrap();
        for path in ["a.md", "b.md"] {
            db.replace_projection_source(
                serde_json::from_value(source(projection, path, path)).unwrap(),
            )
            .await
            .unwrap();
        }
    }
    let error = json!({"projectionId":"books/book","sourcePath":"a.md","sourceHash":"new-hash","error":"parse failure","writerPluginId":"books"});
    db.mark_projection_source_error(serde_json::from_value(error).unwrap())
        .await
        .unwrap();
    db.close().await.unwrap();
    assert_eq!(inspect(profile.path(), id, "SELECT source_hash,status,error FROM index_projection_sources WHERE projection_id='books/book' AND source_path='a.md'").await, vec![vec![json!("new-hash"),json!("error"),json!("parse failure")]]);
    for table in [
        "index_projection_rows",
        "index_projection_values",
        "index_projection_edges",
    ] {
        assert_eq!(
            inspect(
                profile.path(),
                id,
                &format!("SELECT COUNT(*) FROM {table} WHERE projection_id='books/book'")
            )
            .await[0][0],
            json!(if table.ends_with("values") { 7 } else { 1 })
        );
    }
    let db = Database::open(profile.path(), id).await.unwrap();
    db.delete_projection_source("books/book", "b.md", Some("books"))
        .await
        .unwrap();
    db.unregister_projection_definition("other/book")
        .await
        .unwrap();
    db.close().await.unwrap();
    for table in [
        "index_projection_rows",
        "index_projection_values",
        "index_projection_edges",
    ] {
        assert_eq!(
            inspect(profile.path(), id, &format!("SELECT COUNT(*) FROM {table}")).await[0][0],
            json!(0)
        );
    }
    assert_eq!(
        inspect(
            profile.path(),
            id,
            "SELECT projection_id,active FROM index_projections ORDER BY projection_id"
        )
        .await,
        vec![
            vec![json!("books/book"), json!(1)],
            vec![json!("other/book"), json!(0)]
        ]
    );
}
