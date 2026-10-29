use super::{Arguments, Database, done, optional, value};
use crate::services::Result;
use lapis_app_protocol::database::call::{DatabaseMethod, DatabaseMethod::*, DatabaseValue};

pub(super) async fn call(
    db: &Database,
    method: DatabaseMethod,
    a: &Arguments,
) -> Result<DatabaseValue> {
    match method {
        GetMemorySourceState => optional(db.get_memory_source_state(a.text(0)?).await),
        ListMemorySourceStates => value(db.list_memory_source_states().await),
        UpsertMemorySourceState => done(db.upsert_memory_source_state(a.required(0)?).await),
        DeleteMemorySourceState => done(db.delete_memory_source_state(a.text(0)?).await),
        UpsertMemoryCandidate => done(db.upsert_memory_candidate(a.required(0)?).await),
        QueryMemoryCandidates => value(db.query_memory_candidates(a.default(0)?).await),
        DeleteMemoryCandidatesByConversation => done(
            db.delete_memory_candidates_by_conversation(a.text(0)?)
                .await,
        ),
        RecordMemoryRecallSignal => done(db.record_memory_recall_signal(a.required(0)?).await),
        ListMemoryRecallSignals => value(db.list_memory_recall_signals(a.optional_text(0)?).await),
        ClaimMemoryJob => value(db.claim_memory_job(a.required(0)?).await),
        UpdateMemoryJob => value(db.update_memory_job(a.required(0)?).await),
        ListMemoryJobs => value(db.list_memory_jobs(a.optional_text(0)?).await),
        ClearMemoryDerivedState => done(db.clear_memory_derived_state().await),
        _ => unreachable!("call router selects the memory domain"),
    }
}
