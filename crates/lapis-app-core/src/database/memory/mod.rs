mod candidates;
mod client;
mod jobs;
mod reads;
mod writes;
use crate::database::{encoded, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::memory::{jobs::*, records::*};
use serde::Serialize;

#[derive(Serialize)]
pub(super) enum Action {
    Source(String),
    Sources,
    PutSource(MemorySource),
    DeleteSource(String),
    PutCandidate(MemoryCandidateInput),
    Candidates(MemoryCandidateQuery),
    DeleteConversation(String),
    Signal(MemoryRecallSignal),
    Signals(Option<String>),
    Claim(MemoryJobClaim),
    Update(MemoryJobUpdate),
    Jobs(Option<String>),
    Clear,
}
pub(super) enum Value {
    Done,
    Source(Option<MemorySource>),
    Sources(Vec<MemorySource>),
    Candidates(Vec<MemoryCandidateInput>),
    Signals(Vec<MemoryRecallSignal>),
    Job(Option<MemoryJob>),
    Jobs(Vec<MemoryJob>),
}
impl Store {
    pub async fn memory(&mut self, action: Action) -> Result<Value> {
        match action {
            Action::Source(key) => self.memory_source(&key).await.map(Value::Source),
            Action::Sources => self
                .memory_rows(
                    "SELECT data_json FROM memory_source_state ORDER BY source_key",
                    vec![],
                )
                .await
                .map(Value::Sources),
            Action::Candidates(query) => self.memory_candidates(query).await.map(Value::Candidates),
            Action::Signals(target) => self
                .memory_signals(target.as_deref())
                .await
                .map(Value::Signals),
            Action::Jobs(scope) => {
                let (filter, args) = filter("scope_key", scope.as_deref());
                self.memory_rows(
                    &format!(
                        "SELECT data_json FROM memory_jobs{filter} ORDER BY created_at DESC,id"
                    ),
                    args,
                )
                .await
                .map(Value::Jobs)
            }
            write => self.mutate_memory(write).await,
        }
    }
}
fn filter(column: &str, value: Option<&str>) -> (String, Vec<turso::Value>) {
    match value {
        Some(v) => (format!(" WHERE {column}=?"), vec![text(v)]),
        None => (String::new(), vec![]),
    }
}
fn text(value: impl Into<String>) -> turso::Value {
    turso::Value::Text(value.into())
}
fn enum_text(value: &impl Serialize) -> Result<String> {
    serde_json::to_value(value)
        .map_err(|_| invalid("Invalid memory enum"))?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("Invalid memory enum"))
}
fn json(value: &impl Serialize) -> Result<String> {
    String::from_utf8(encoded(value)?).map_err(|_| invalid("Invalid memory JSON"))
}
