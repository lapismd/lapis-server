use lapis_app_core::database::Database;
use lapis_app_protocol::{
    ErrorCode,
    database::memory::{jobs::*, records::*},
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{future::Future, task::Poll};
use uuid::Uuid;
mod indexed_support;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/memory-storage.json"
    ))
    .unwrap()
}
fn data<T: DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}
fn claim(owner: &str) -> MemoryJobClaim {
    data(json!({"job":fixture()["job"],"ownerId":owner,"now":200,"leaseMs":100}))
}

#[tokio::test]
async fn competing_clients_get_one_claim_and_cancelled_waiters_still_commit_before_close() {
    let dir = tempfile::tempdir().unwrap();
    let vault = Uuid::new_v4();
    let db = Database::open(dir.path(), vault).await.unwrap();
    let mut handles = vec![];
    for i in 0..16 {
        let db = db.clone();
        handles.push(tokio::spawn(async move {
            db.claim_memory_job(claim(&format!("client-{i}")))
                .await
                .unwrap()
        }));
    }
    let mut winners = 0;
    for h in handles {
        winners += usize::from(h.await.unwrap().is_some());
    }
    assert_eq!(winners, 1);
    assert_eq!(db.revision().await.unwrap(), 1);
    let mut input = claim("detached");
    input.job.id = "detached".into();
    let mut pending = Box::pin(db.claim_memory_job(input));
    std::future::poll_fn(|cx| {
        assert!(pending.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(pending);
    db.close().await.unwrap();
    let db = Database::open(dir.path(), vault).await.unwrap();
    let jobs = db.list_memory_jobs(None).await.unwrap();
    assert_eq!(jobs.len(), 2);
    assert_eq!(
        jobs.iter()
            .find(|j| j.id == "detached")
            .unwrap()
            .owner_id
            .as_deref(),
        Some("detached")
    );
    assert_eq!(db.revision().await.unwrap(), 2);
    db.close().await.unwrap();
}

#[tokio::test]
async fn duplicate_origins_roll_back_replacement_and_memory_clear_preserves_other_domains() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path(), Uuid::new_v4()).await.unwrap();
    let original: MemoryCandidateInput = data(fixture()["candidates"][0].clone());
    db.upsert_memory_candidate(original.clone()).await.unwrap();
    db.upsert_indexed_metadata(indexed_support::record("note.md", 1))
        .await
        .unwrap();
    db.set_notebook_state(
        "note.md",
        data(json!({"sourcePath":"note.md","mtime":1,"updatedAt":2,"cells":{}})),
    )
    .await
    .unwrap();
    db.upsert_notification(data(json!({"id":"n","message":"retained","severity":"info","createdAt":1,"updatedAt":1,"read":false,"cleared":false}))).await.unwrap();
    db.store_file_history_revision(data(
        json!({"path":"note.md","eventType":"create","createdAt":1,"content":"retained"}),
    ))
    .await
    .unwrap();
    let history = serde_json::to_value(db.get_file_history("note.md").await.unwrap()).unwrap();
    let mut changed = original.clone();
    changed.candidate.normalized_claim = "must roll back".into();
    changed.origins.push(original.origins[0].clone());
    let before = db.revision().await.unwrap();
    let mut events = db.subscribe_to_changes();
    assert_eq!(
        db.upsert_memory_candidate(changed)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Io
    );
    assert_eq!(db.revision().await.unwrap(), before);
    assert!(events.try_recv().is_err());
    let restored = db
        .query_memory_candidates(Default::default())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::to_value([original]).unwrap()
    );
    db.delete_memory_candidates_by_conversation("missing")
        .await
        .unwrap();
    assert_eq!(db.revision().await.unwrap(), before);
    assert!(events.try_recv().is_err());
    db.clear_memory_derived_state().await.unwrap();
    assert_eq!(
        serde_json::to_value(db.get_file_history("note.md").await.unwrap()).unwrap(),
        history
    );
    assert!(db.get_notebook_state("note.md").await.unwrap().is_some());
    assert!(db.get_indexed_file("note.md").await.unwrap().is_some());
    assert_eq!(db.list_notifications().await.unwrap().len(), 1);
    let event = events.try_recv().unwrap();
    assert_eq!(event.revision, before + 1);
    assert_eq!(
        serde_json::to_value(event.domains).unwrap(),
        json!(["memory"])
    );
    assert!(events.try_recv().is_err());
    db.close().await.unwrap();
}

#[tokio::test]
async fn bounded_attempts_cannot_restart_failed_jobs_and_zero_duration_gets_one_tick() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path(), Uuid::new_v4()).await.unwrap();
    let mut input = claim("a");
    input.job.max_attempts = 1.;
    input.lease_ms = 0.;
    let first = db.claim_memory_job(input.clone()).await.unwrap().unwrap();
    assert_eq!(first.lease_until, Some(201.));
    assert_eq!(
        db.claim_memory_job(input.clone())
            .await
            .unwrap()
            .unwrap()
            .attempts,
        1.
    );
    db.update_memory_job(data(json!({"jobId":input.job.id,"ownerId":"a","now":201,"patch":{"status":"failed","errorCode":"source-unavailable","finishedAt":201}}))).await.unwrap().unwrap();
    let before = db.revision().await.unwrap();
    let mut events = db.subscribe_to_changes();
    input.now = 202.;
    input.owner_id = "b".into();
    assert!(db.claim_memory_job(input).await.unwrap().is_none());
    assert_eq!(db.revision().await.unwrap(), before);
    assert!(events.try_recv().is_err());
    db.close().await.unwrap();
}

#[tokio::test]
async fn invalid_nested_numbers_and_importance_reject_before_memory_admission() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path(), Uuid::new_v4()).await.unwrap();
    let mut events = db.subscribe_to_changes();
    let mut source: MemorySource = data(fixture()["source"].clone());
    source.indexed_at = f64::NAN;
    assert_eq!(
        db.upsert_memory_source_state(source)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    let base: MemoryCandidateInput = data(fixture()["candidates"][0].clone());
    let mut bad = base.clone();
    bad.candidate.last_seen_at = f64::INFINITY;
    let mut bad_origin = base.clone();
    bad_origin.origins[0].observed_at = f64::NAN;
    let mut bad_priority = base;
    bad_priority.candidate.importance = 0;
    for record in [bad, bad_origin, bad_priority] {
        assert_eq!(
            db.upsert_memory_candidate(record).await.unwrap_err().code(),
            ErrorCode::InvalidRequest
        );
    }
    let signal = MemoryRecallSignal {
        target_ref: "x".into(),
        query_fingerprint: "salted".into(),
        day: "today".into(),
        best_score: f64::NAN,
        hit_count: 1.,
    };
    assert_eq!(
        db.record_memory_recall_signal(signal)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    let mut bad_job = claim("a");
    bad_job.job.attempts = f64::INFINITY;
    assert_eq!(
        db.claim_memory_job(bad_job).await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    let mut update: MemoryJobUpdate =
        data(json!({"jobId":"x","ownerId":"a","now":1,"patch":{"status":"completed"}}));
    update.patch.finished_at = Some(f64::NAN);
    assert_eq!(
        db.update_memory_job(update).await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(db.revision().await.unwrap(), 0);
    assert!(events.try_recv().is_err());
    db.close().await.unwrap();
}
