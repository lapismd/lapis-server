use lapis_app_core::database::Database;
use lapis_app_protocol::{
    ErrorCode,
    database::memory::records::{MemoryCandidateInput, MemoryCandidateQuery},
};
use serde_json::{Value, json};
use uuid::Uuid;
mod projection_support;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/memory-storage.json"
    ))
    .unwrap()
}

#[tokio::test]
async fn origin_materialization_crosses_bind_chunks_and_replacement_filters_foreign_origins() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path(), Uuid::new_v4()).await.unwrap();
    let base: MemoryCandidateInput =
        serde_json::from_value(fixture()["candidates"][0].clone()).unwrap();
    for i in 0..130 {
        let mut input = base.clone();
        input.candidate.id = format!("candidate-{i:03}");
        input.candidate.last_seen_at = i as f64;
        input.origins.truncate(1);
        input.origins[0].candidate_id = input.candidate.id.clone();
        input.origins[0].entry_id = format!("entry-{i}");
        input.origins.push(base.origins[1].clone()); // belongs to a different candidate
        db.upsert_memory_candidate(input).await.unwrap();
    }
    let rows = db
        .query_memory_candidates(MemoryCandidateQuery {
            limit: Some(129.),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(rows.len(), 129);
    for (i, row) in (1..130).rev().zip(rows) {
        assert_eq!(row.candidate.id, format!("candidate-{i:03}"));
        assert_eq!(row.origins.len(), 1);
        assert_eq!(row.origins[0].candidate_id, row.candidate.id);
        assert_eq!(row.origins[0].entry_id, format!("entry-{i}"));
    }
    assert_eq!(
        db.query_memory_candidates(MemoryCandidateQuery {
            limit: Some(2.5),
            ..Default::default()
        })
        .await
        .unwrap_err()
        .code(),
        ErrorCode::InvalidRequest
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn memory_schema_failure_rolls_back_its_additive_tables_and_preserves_prior_domains() {
    let dir = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(dir.path(), id).await.unwrap();
    db.set_meta("unrelated", Some(json!("retained")))
        .await
        .unwrap();
    db.close().await.unwrap();
    let sql = turso::Builder::new_local(dir.path().join(format!("{id}.db")).to_str().unwrap())
        .build()
        .await
        .unwrap();
    let conn = sql.connect().unwrap();
    conn.execute_batch("DROP TABLE memory_source_state; DROP TABLE memory_candidates; DROP TABLE memory_candidate_origins; DROP TABLE memory_recall_signals; DROP TABLE memory_jobs; CREATE TABLE memory_candidate_origins(candidate_id TEXT)").await.unwrap();
    let mut checkpoint = conn
        .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
        .await
        .unwrap();
    assert_eq!(
        checkpoint
            .next()
            .await
            .unwrap()
            .unwrap()
            .get::<i64>(0)
            .unwrap(),
        0
    );
    drop(checkpoint);
    drop(conn);
    drop(sql);
    assert_eq!(
        Database::open(dir.path(), id).await.err().unwrap().code(),
        ErrorCode::Io
    );
    let tables = projection_support::inspect(
        dir.path(),
        id,
        "SELECT name FROM sqlite_schema WHERE type='table' AND name LIKE 'memory_%' ORDER BY name",
    )
    .await;
    assert_eq!(tables, vec![vec![json!("memory_candidate_origins")]]);
    let retained = projection_support::inspect(
        dir.path(),
        id,
        "SELECT value_json FROM app_meta WHERE key='unrelated'",
    )
    .await;
    assert_eq!(retained, vec![vec![json!("\"retained\"")]]);
}
