use lapis_app_core::database::Database;
use lapis_app_protocol::database::indexed::queries::*;
use serde_json::json;
use uuid::Uuid;

mod indexed_support;
use indexed_support::record;

#[tokio::test]
async fn native_indexed_metadata_retains_records_manifest_and_selected_domains_after_reopen() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    db.upsert_indexed_metadata(record("notes/a.md", 3))
        .await
        .unwrap();
    let saved = db.get_indexed_file("notes/a.md").await.unwrap().unwrap();
    assert_eq!(saved.file.extension, "md");
    assert_eq!(saved.links[0].ordinal, Some(0.0));
    let manifest = db
        .list_indexed_file_manifest(ManifestQuery::default())
        .await
        .unwrap();
    assert_eq!(manifest.rows.len(), 1);
    assert_eq!(manifest.rows[0].parser_version.as_deref(), Some("parser-1"));
    let page = db
        .query_indexed_metadata_page(serde_json::from_value(json!({"include":[]})).unwrap())
        .await
        .unwrap();
    assert!(
        page.rows[0].metadata.is_none()
            && page.rows[0].properties.is_empty()
            && page.rows[0].links.is_empty()
            && page.rows[0].tags.is_empty()
    );
    assert_eq!(db.revision().await.unwrap(), 1);
    db.close().await.unwrap();
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(
        db.get_indexed_file("notes/a.md")
            .await
            .unwrap()
            .unwrap()
            .properties
            .len(),
        3
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn indexed_filters_use_nested_values_tags_prefixes_hidden_paths_and_stable_pages() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    for (path, score) in [
        ("notes/a.md", 1),
        ("notes/b.md", 2),
        ("notes-other/c.md", 3),
        (".lapis/hidden.md", 4),
    ] {
        db.upsert_indexed_metadata(record(path, score))
            .await
            .unwrap();
    }
    let query:MetadataQuery=serde_json::from_value(json!({"extensions":[".MD"],"pathPrefixes":["/notes/"],"requiredTags":["#work"],"propertyFilters":[{"name":"team.members[].active","op":"=","value":true},{"name":"score","op":">","value":1}]})).unwrap();
    let rows = db.query_indexed_metadata(query).await.unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r.file.path.as_str())
            .collect::<Vec<_>>(),
        ["notes/b.md"]
    );
    let first = db
        .query_indexed_metadata_page(
            serde_json::from_value(json!({"query":{"excludeHiddenPaths":true},"limit":1})).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.rows[0].file.path, "notes-other/c.md");
    let second = db
        .query_indexed_metadata_page(
            serde_json::from_value(
                json!({"query":{"excludeHiddenPaths":true},"after":first.next_cursor,"limit":1}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.rows[0].file.path, "notes/a.md");
    db.close().await.unwrap();
}

#[tokio::test]
async fn native_facets_and_link_queries_retain_typed_normalized_data() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    db.upsert_indexed_metadata(record("notes/a.md", 3))
        .await
        .unwrap();
    let facets = db
        .query_metadata_facets(serde_json::from_value(json!({"kind":"tag"})).unwrap())
        .await
        .unwrap();
    assert_eq!(facets.len(), 2);
    let values = db
        .query_metadata_facets(
            serde_json::from_value(
                json!({"kind":"property-value","propertyName":"team.members[].active"}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&values).unwrap()[0]["value"],
        json!(true)
    );
    let incoming = db
        .query_metadata_links(
            serde_json::from_value(
                json!({"direction":"incoming","path":"target.md","resolution":"resolved"}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].count, 2.0);
    assert!(
        db.query_metadata_links(
            serde_json::from_value(
                json!({"direction":"outgoing","path":"notes/a.md","resolution":"unresolved"})
            )
            .unwrap()
        )
        .await
        .unwrap()
        .is_empty()
    );
    db.close().await.unwrap();
}
