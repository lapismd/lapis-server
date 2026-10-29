use super::{Action, Value};
use crate::database::{Database, Operation, Output, invalid};
use crate::services::Result;
use lapis_app_protocol::database::memory::{jobs::*, records::*};

impl Database {
    pub async fn get_memory_source_state(&self, key: &str) -> Result<Option<MemorySource>> {
        match self
            .submit(Operation::Memory(Action::Source(key.into())))
            .await?
        {
            Output::Memory(Value::Source(row)) => Ok(row),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn list_memory_source_states(&self) -> Result<Vec<MemorySource>> {
        match self.submit(Operation::Memory(Action::Sources)).await? {
            Output::Memory(Value::Sources(rows)) => Ok(rows),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn upsert_memory_source_state(&self, record: MemorySource) -> Result<()> {
        finite(&[record.indexed_at])?;
        self.submit(Operation::Memory(Action::PutSource(record)))
            .await
            .map(|_| ())
    }
    pub async fn delete_memory_source_state(&self, key: &str) -> Result<()> {
        self.submit(Operation::Memory(Action::DeleteSource(key.into())))
            .await
            .map(|_| ())
    }
    pub async fn upsert_memory_candidate(&self, input: MemoryCandidateInput) -> Result<()> {
        let c = &input.candidate;
        if !(1..=5).contains(&c.importance) {
            return Err(invalid("Invalid memory importance"));
        }
        finite(&[
            c.first_seen_at,
            c.last_seen_at,
            c.recurrence_count,
            c.conversation_count,
        ])?;
        for origin in &input.origins {
            finite(&[origin.observed_at])?;
        }
        self.submit(Operation::Memory(Action::PutCandidate(input)))
            .await
            .map(|_| ())
    }
    pub async fn query_memory_candidates(
        &self,
        query: MemoryCandidateQuery,
    ) -> Result<Vec<MemoryCandidateInput>> {
        if let Some(limit) = query.limit {
            finite(&[limit])?;
        }
        match self
            .submit(Operation::Memory(Action::Candidates(query)))
            .await?
        {
            Output::Memory(Value::Candidates(rows)) => Ok(rows),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn delete_memory_candidates_by_conversation(&self, id: &str) -> Result<()> {
        self.submit(Operation::Memory(Action::DeleteConversation(id.into())))
            .await
            .map(|_| ())
    }
    pub async fn record_memory_recall_signal(&self, record: MemoryRecallSignal) -> Result<()> {
        finite(&[record.best_score, record.hit_count])?;
        self.submit(Operation::Memory(Action::Signal(record)))
            .await
            .map(|_| ())
    }
    pub async fn list_memory_recall_signals(
        &self,
        target: Option<&str>,
    ) -> Result<Vec<MemoryRecallSignal>> {
        match self
            .submit(Operation::Memory(Action::Signals(
                target.map(str::to_owned),
            )))
            .await?
        {
            Output::Memory(Value::Signals(rows)) => Ok(rows),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn claim_memory_job(&self, input: MemoryJobClaim) -> Result<Option<MemoryJob>> {
        valid_job(&input.job)?;
        finite(&[
            input.now,
            input.lease_ms,
            input.now + input.lease_ms.max(1.),
        ])?;
        match self.submit(Operation::Memory(Action::Claim(input))).await? {
            Output::Memory(Value::Job(row)) => Ok(row),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn update_memory_job(&self, input: MemoryJobUpdate) -> Result<Option<MemoryJob>> {
        let mut values = vec![input.now];
        values.extend(input.patch.lease_until);
        values.extend(input.patch.finished_at);
        finite(&values)?;
        match self
            .submit(Operation::Memory(Action::Update(input)))
            .await?
        {
            Output::Memory(Value::Job(row)) => Ok(row),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn list_memory_jobs(&self, scope: Option<&str>) -> Result<Vec<MemoryJob>> {
        match self
            .submit(Operation::Memory(Action::Jobs(scope.map(str::to_owned))))
            .await?
        {
            Output::Memory(Value::Jobs(rows)) => Ok(rows),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn clear_memory_derived_state(&self) -> Result<()> {
        self.submit(Operation::Memory(Action::Clear))
            .await
            .map(|_| ())
    }
}
pub(super) fn finite(values: &[f64]) -> Result<()> {
    if values.iter().all(|n| n.is_finite()) {
        Ok(())
    } else {
        Err(invalid("Invalid memory numeric data"))
    }
}
pub(super) fn valid_job(job: &MemoryJob) -> Result<()> {
    let mut values = vec![job.attempts, job.max_attempts, job.created_at];
    values.extend(job.lease_until);
    values.extend(job.started_at);
    values.extend(job.finished_at);
    finite(&values)
}
