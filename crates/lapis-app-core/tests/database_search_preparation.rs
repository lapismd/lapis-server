mod search_support;
use lapis_app_core::database::Database;
use lapis_app_protocol::{ErrorCode, database::search::records::SearchDocumentRecord};
use search_support::assert_json;
use serde_json::{Value, json};
use uuid::Uuid;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/search-preparation.json"
    ))
    .unwrap()
}
#[tokio::test]
async fn native_preparation_retains_all_source_golden_records_and_utf16_offsets() {
    let directory = tempfile::tempdir().unwrap();
    let db = Database::open(directory.path(), Uuid::new_v4())
        .await
        .unwrap();
    for case in fixture()["cases"].as_array().unwrap() {
        let input: SearchDocumentRecord = serde_json::from_value(case["input"].clone()).unwrap();
        db.upsert_search_document(input.clone()).await.unwrap();
        let actual = db.get_search_document(&input.path).await.unwrap().unwrap();
        eprintln!("reference case: {}", case["name"]);
        assert_json(
            serde_json::to_value(&actual).unwrap(),
            case["expected"].clone(),
        );
        if input.source_metadata.is_some() {
            let units: Vec<_> = input.content.encode_utf16().collect();
            for chunk in actual.chunks.unwrap() {
                assert_eq!(
                    chunk.text.encode_utf16().collect::<Vec<_>>(),
                    units[chunk.start_offset as usize..chunk.end_offset as usize]
                );
            }
        }
    }
    db.close().await.unwrap();
}
#[tokio::test]
async fn preparation_rejects_invalid_settings_without_replacing_committed_storage() {
    let directory = tempfile::tempdir().unwrap();
    let db = Database::open(directory.path(), Uuid::new_v4())
        .await
        .unwrap();
    let input = fixture()["cases"][7]["input"].clone();
    let valid: SearchDocumentRecord = serde_json::from_value(input.clone()).unwrap();
    db.upsert_search_document(valid).await.unwrap();
    let baseline = db.get_search_document("Note.md").await.unwrap();
    let revision = db.revision().await.unwrap();
    for (field, value) in [
        ("targetChars", json!(0)),
        ("targetChars", json!(-1)),
        ("targetChars", json!(1.5)),
        ("breakpointWindowChars", json!(-1)),
    ] {
        let mut invalid = input.clone();
        invalid["sourceMetadata"]["chunking"][field] = value;
        let error = db
            .upsert_search_document(serde_json::from_value(invalid).unwrap())
            .await
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidRequest);
        assert_eq!(db.revision().await.unwrap(), revision);
        assert_json(
            serde_json::to_value(db.get_search_document("Note.md").await.unwrap()).unwrap(),
            serde_json::to_value(&baseline).unwrap(),
        );
    }
    db.close().await.unwrap();
}

#[tokio::test]
async fn oversized_preparation_and_invalid_source_offsets_preserve_storage_and_admission() {
    let directory = tempfile::tempdir().unwrap();
    let db = Database::open(directory.path(), Uuid::new_v4())
        .await
        .unwrap();
    let input = fixture()["cases"][7]["input"].clone();
    db.upsert_search_document(serde_json::from_value(input.clone()).unwrap())
        .await
        .unwrap();
    let revision = db.revision().await.unwrap();
    let baseline = db.get_search_document("Note.md").await.unwrap();
    for offset in [json!(3), json!(-1), json!(0.5)] {
        let mut invalid = input.clone();
        invalid["sourceMetadata"]["frontmatterEndOffset"] = offset;
        assert_eq!(
            db.upsert_search_document(serde_json::from_value(invalid).unwrap())
                .await
                .unwrap_err()
                .code(),
            ErrorCode::InvalidRequest
        );
    }
    let mut huge = input.clone();
    huge["content"] = json!("x".repeat(400_000));
    huge["sourceMetadata"]["chunking"]["targetChars"] = json!(1);
    assert_eq!(
        db.upsert_search_document(serde_json::from_value(huge).unwrap())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(db.revision().await.unwrap(), revision);
    assert_json(
        serde_json::to_value(db.get_search_document("Note.md").await.unwrap()).unwrap(),
        serde_json::to_value(baseline).unwrap(),
    );
    db.upsert_search_document(serde_json::from_value(input.clone()).unwrap())
        .await
        .unwrap();
    db.close().await.unwrap();
    assert_eq!(
        db.upsert_search_document(serde_json::from_value(input).unwrap())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Closed
    );
}
