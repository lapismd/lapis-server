use lapis_app_core::database::Database;
use lapis_app_protocol::database::{changes::ChangeDomain, search::SearchManifestQuery};
use serde_json::{Value, json};
use uuid::Uuid;
mod search_support;
use search_support::{assert_json, fixture, seed};

#[tokio::test]
async fn native_search_storage_retains_the_complete_provider_fixture_and_cold_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(dir.path(), id).await.unwrap();
    let mut changes = db.subscribe_to_changes();
    seed(&db).await;
    for r in fixture()["records"].as_array().unwrap() {
        let change = changes.try_recv().unwrap();
        assert!(matches!(change.domains.as_slice(), [ChangeDomain::Search]));
        assert_eq!(change.paths, vec![r["stored"]["path"].as_str().unwrap()]);
        assert_json(
            serde_json::to_value(
                db.get_search_document(r["stored"]["path"].as_str().unwrap())
                    .await
                    .unwrap()
                    .unwrap(),
            )
            .unwrap(),
            r["stored"].clone(),
        );
    }
    assert!(db.get_search_document("missing").await.unwrap().is_none());
    assert_json(
        serde_json::to_value(db.get_search_index_stats().await.unwrap()).unwrap(),
        fixture()["stats"].clone(),
    );
    assert_eq!(db.revision().await.unwrap(), 4);
    assert!(changes.try_recv().is_err());
    db.close().await.unwrap();
    let db = Database::open(dir.path(), id).await.unwrap();
    assert_json(
        serde_json::to_value(db.list_search_documents().await.unwrap()).unwrap(),
        Value::Array(
            fixture()["records"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["stored"].clone())
                .collect(),
        ),
    );
    assert_json(
        serde_json::to_value(db.get_search_index_stats().await.unwrap()).unwrap(),
        fixture()["stats"].clone(),
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn native_search_manifests_keep_body_free_path_pages_and_optional_source_columns() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path(), Uuid::new_v4()).await.unwrap();
    seed(&db).await;
    let mut collected = vec![];
    let mut after = None;
    loop {
        let page = db
            .list_search_document_manifest(SearchManifestQuery {
                after,
                limit: Some(1),
            })
            .await
            .unwrap();
        assert_eq!(page.rows.len(), 1);
        for row in &page.rows {
            let v = serde_json::to_value(row).unwrap();
            assert!(v.get("content").is_none());
            assert!(v.get("chunks").is_none());
        }
        collected.extend(page.rows);
        after = page.next_cursor;
        if after.is_none() {
            break;
        }
    }
    assert_json(
        serde_json::to_value(collected).unwrap(),
        fixture()["manifest"].clone(),
    );
    let page = db
        .list_search_document_manifest(SearchManifestQuery {
            after: Some("Z.md".into()),
            limit: None,
        })
        .await
        .unwrap();
    assert!(page.rows.is_empty());
    assert!(page.next_cursor.is_none());
    db.close().await.unwrap();
}

#[tokio::test]
async fn native_search_replacement_and_delete_remove_old_chunk_rows_and_source_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path(), Uuid::new_v4()).await.unwrap();
    seed(&db).await;
    let replacement = fixture()["replacement"].clone();
    db.upsert_prepared_search_document(serde_json::from_value(replacement.clone()).unwrap())
        .await
        .unwrap();
    assert_json(
        serde_json::to_value(db.get_search_document("B.md").await.unwrap().unwrap()).unwrap(),
        replacement,
    );
    let manifest = db
        .list_search_document_manifest(Default::default())
        .await
        .unwrap();
    assert_json(
        serde_json::to_value(manifest.rows.iter().find(|r| r.path == "B.md").unwrap()).unwrap(),
        json!({"path":"B.md","checksum":"replaced"}),
    );
    db.delete_search_document("A.md").await.unwrap();
    assert!(db.get_search_document("A.md").await.unwrap().is_none());
    assert_json(
        serde_json::to_value(db.get_search_index_stats().await.unwrap()).unwrap(),
        json!({"documentCount":3,"chunkCount":0,"readyChunkCount":0,"pendingChunkCount":0,"errorChunkCount":0,"lastError":null}),
    );
    db.close().await.unwrap();
}
