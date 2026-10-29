use lapis_app_core::database::Database;
use lapis_app_protocol::{
    ErrorCode,
    database::indexed::{queries::*, records::PropertyRecord},
};
use serde_json::json;
use uuid::Uuid;
mod indexed_support;
use indexed_support::record;

#[tokio::test]
async fn scalar_filters_sort_facets_and_replacement_retain_normalized_semantics() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    for score in 1..=3 {
        db.upsert_indexed_metadata(record(&format!("n{score}.md"), score))
            .await
            .unwrap();
    }
    let query:MetadataQuery=serde_json::from_value(json!({"propertyFilters":[{"name":"due","op":">=","value":"2026-10-24"},{"name":"team.members[0].name","op":"exists"},{"name":"absent","op":"not-exists"}],"sort":[{"field":{"kind":"property","name":"Score"},"direction":"DESC"}]})).unwrap();
    let rows = db.query_indexed_metadata(query).await.unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r.file.path.as_str())
            .collect::<Vec<_>>(),
        ["n3.md", "n2.md", "n1.md"]
    );
    let names = db
        .query_metadata_facets(serde_json::from_value(json!({"kind":"property-name"})).unwrap())
        .await
        .unwrap();
    assert_eq!(names.len(), 3);
    assert!(
        names
            .iter()
            .all(|r| r.count == 3 && r.top_level == Some(true))
    );
    let paths = db
        .query_metadata_facets(serde_json::from_value(json!({"kind":"property-path"})).unwrap())
        .await
        .unwrap();
    assert!(paths.iter().any(
        |r| matches!(&r.value,MetadataScalar::String(s) if s=="Team.members[].name")
            && r.top_level == Some(false)
    ));
    let mut replacement = record("n1.md", 4);
    replacement.tags.clear();
    replacement.links.clear();
    replacement.properties.clear();
    db.upsert_indexed_metadata(replacement).await.unwrap();
    let only = db.get_indexed_file("n1.md").await.unwrap().unwrap();
    assert!(only.tags.is_empty() && only.properties.is_empty() && only.links.is_empty());
    let tags = db
        .query_metadata_facets(serde_json::from_value(json!({"kind":"tag"})).unwrap())
        .await
        .unwrap();
    assert!(tags.iter().all(|r| r.count == 2));
    db.close().await.unwrap();
}

#[tokio::test]
async fn literal_prefixes_and_large_prefix_sets_preserve_path_boundaries() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    for path in ["folder_%/a.md", "folderXY/b.md"] {
        db.upsert_indexed_metadata(record(path, 1)).await.unwrap();
    }
    let mut prefixes = (0..1999)
        .map(|n| format!("missing/{n}"))
        .collect::<Vec<_>>();
    prefixes.push("/folder_%/".into());
    let query = MetadataQuery {
        path_prefixes: prefixes,
        ..Default::default()
    };
    let rows = db.query_indexed_metadata(query).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].file.path, "folder_%/a.md");
    let none = db
        .query_indexed_metadata(
            serde_json::from_value(
                json!({"propertyFilters":[{"name":"score'); DROP TABLE files; --","op":"exists"}]}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(none.is_empty());
    assert_eq!(
        db.list_indexed_file_manifest(ManifestQuery::default())
            .await
            .unwrap()
            .rows
            .len(),
        2
    );
    assert!(serde_json::from_value::<MetadataQuery>(json!({"sort":[{"field":{"kind":"file","field":"path; DROP TABLE files"},"direction":"ASC"}]})).is_err());
    db.close().await.unwrap();
}

#[tokio::test]
async fn selected_queries_and_facets_use_typed_columns_without_loading_corrupt_metadata_json() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    db.upsert_indexed_metadata(record("note.md", 3))
        .await
        .unwrap();
    db.close().await.unwrap();
    let engine =
        turso::Builder::new_local(profile.path().join(format!("{id}.db")).to_str().unwrap())
            .build()
            .await
            .unwrap();
    let sql = engine.connect().unwrap();
    sql.execute(
        "UPDATE metadata SET data_json='{broken' WHERE path='note.md'",
        (),
    )
    .await
    .unwrap();
    drop(sql);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    let page=db.query_indexed_metadata_page(serde_json::from_value(json!({"query":{"propertyFilters":[{"name":"score","op":"=","value":3}]},"include":[]})).unwrap()).await.unwrap();
    assert_eq!(page.rows.len(), 1);
    assert!(page.rows[0].metadata.is_none());
    let facets = db
        .query_metadata_facets(
            serde_json::from_value(json!({"kind":"property-value","propertyName":"Score"}))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(facets[0].value,MetadataScalar::Number(n) if n==3.0));
    assert_eq!(
        db.get_indexed_file("note.md").await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(db.revision().await.unwrap(), 1);
    db.close().await.unwrap();
}

#[tokio::test]
async fn indexed_publication_failure_retains_previous_file_and_all_normalized_domains() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let engine =
        turso::Builder::new_local(profile.path().join(format!("{id}.db")).to_str().unwrap())
            .build()
            .await
            .unwrap();
    let sql = engine.connect().unwrap();
    sql.execute_batch("CREATE TABLE metadata_properties(path TEXT NOT NULL,ordinal INTEGER NOT NULL,name TEXT NOT NULL CHECK(name!='reject'),normalized_name TEXT NOT NULL,inferred_type TEXT NOT NULL,declared_type TEXT,data_json TEXT NOT NULL,PRIMARY KEY(path,ordinal));").await.unwrap();
    drop(sql);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    let mut events = db.subscribe_to_changes();
    db.upsert_indexed_metadata(record("note.md", 1))
        .await
        .unwrap();
    assert_eq!(events.recv().await.unwrap().revision, 1);
    let mut failed = record("note.md", 2);
    failed.properties.push(PropertyRecord {
        path: "note.md".into(),
        name: "reject".into(),
        inferred_type: "string".into(),
        declared_type: None,
        value: json!("bad"),
    });
    assert_eq!(
        db.upsert_indexed_metadata(failed).await.unwrap_err().code(),
        ErrorCode::Io
    );
    assert!(matches!(
        events.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    let retained = db.get_indexed_file("note.md").await.unwrap().unwrap();
    assert_eq!(retained.file.hash, "hash-1");
    assert_eq!(retained.properties.len(), 3);
    assert_eq!(retained.tags.len(), 1);
    assert_eq!(retained.links.len(), 1);
    assert_eq!(db.revision().await.unwrap(), 1);
    db.close().await.unwrap();
}

#[tokio::test]
async fn batched_materialization_manifest_restriction_and_null_positions_preserve_all_rows() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    for i in 0..405 {
        let mut record = record(&format!("notes/{i:03}.md"), i);
        record.links.clear();
        record.tags.clear();
        record.properties.clear();
        db.upsert_indexed_metadata(record).await.unwrap();
    }
    let rows = db
        .query_indexed_metadata_page(
            serde_json::from_value(json!({"limit":405,"include":["metadata"]})).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rows.rows.len(), 405);
    assert_eq!(rows.rows[404].file.path, "notes/404.md");
    assert!(rows.rows.iter().all(|r| r.metadata.is_some()));
    let manifest = db
        .list_indexed_file_manifest(ManifestQuery {
            paths: vec![
                "notes/001.md".into(),
                "notes/404.md".into(),
                "notes/001.md".into(),
            ],
            limit: Some(1),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(manifest.rows[0].path, "notes/001.md");
    assert_eq!(manifest.next_cursor.as_deref(), Some("notes/001.md"));
    let mut extra = record("null.md", 1);
    extra.links[0].position = Some(json!(null));
    extra.links[0].heading = Some(None);
    extra.tags[0].position = Some(json!(null));
    db.upsert_indexed_metadata(extra).await.unwrap();
    let row = serde_json::to_value(db.get_indexed_file("null.md").await.unwrap().unwrap()).unwrap();
    assert!(row["links"][0].get("position").unwrap().is_null());
    assert!(row["links"][0].get("heading").unwrap().is_null());
    assert!(row["tags"][0].get("position").unwrap().is_null());
    db.close().await.unwrap();
}
