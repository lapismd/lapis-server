//! Private durable dispatch authority. Live vault and execution admission remain required.
mod continuation;
pub mod participant_capture;
pub mod queue_capture;
mod types;
use crate::record_store::RecordStore;
use crate::vault::VaultRoot;
use lapis_app_protocol::turns::{TurnInput, TurnPhase, TurnRecord, TurnSummary};
use std::{io, path::Path};
pub use types::TurnOutcome;
pub(crate) use types::{MAX_OUTPUT_BYTES, normalize};
use types::{MAX_RECORDS, invalid, now};
use uuid::Uuid;

pub struct TurnJournal {
    storage: RecordStore<TurnRecord>,
}

/// Only fresh durable acceptance creates this non-serializable, single-use value.
pub struct DispatchClaim {
    generation: Uuid,
    request_id: Uuid,
    expected: Vec<u8>,
}
pub struct RunningTurn {
    generation: Uuid,
    record: TurnRecord,
    expected: Vec<u8>,
}
pub enum Admission {
    Fresh {
        record: TurnRecord,
        claim: DispatchClaim,
    },
    Existing(TurnRecord),
}

impl TurnJournal {
    pub(crate) fn retire(&mut self) -> io::Result<()> {
        self.storage.retire()
    }
    /// Native installation inputs only. Recovery changes state, never dispatches.
    pub fn open(directory: &Path, vault: Uuid, content: &VaultRoot) -> io::Result<Self> {
        let journal = Self {
            storage: RecordStore::open(directory, vault, content)?,
        };
        for id in journal.storage.ids()? {
            let (mut record, bytes) = journal
                .storage
                .read(id)?
                .ok_or_else(|| invalid("Native turn vanished during recovery"))?;
            let recovery = match record.phase {
                TurnPhase::Accepted => {
                    Some((TurnPhase::Interrupted, "service_restarted_before_dispatch"))
                }
                TurnPhase::Running => {
                    Some((TurnPhase::Unknown, "service_restarted_during_dispatch"))
                }
                _ => None,
            };
            if let Some((phase, reason)) = recovery {
                record.phase = phase;
                record.failure_code = Some(reason.into());
                record.updated_at_ms = record.updated_at_ms.max(now()?);
                journal.storage.write(&record, Some(&bytes))?;
            }
        }
        Ok(journal)
    }
    pub fn accept(&mut self, input: TurnInput) -> io::Result<Admission> {
        let input = normalize(input)?;
        if let Some((record, _)) = self.storage.read(input.request_id)? {
            if record.input != input {
                return Err(invalid(
                    "Native request identity was reused with different input",
                ));
            }
            return Ok(Admission::Existing(record));
        }
        if self.storage.ids()?.len() >= MAX_RECORDS {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Native turn journal is full",
            ));
        }
        let (conversation_id, timestamp) = continuation::select(self, &input)?;
        self.accept_selected(input, conversation_id, timestamp)
    }
    pub(crate) fn plan_participants(
        &self,
        input: &TurnInput,
        count: usize,
    ) -> io::Result<(Uuid, u64)> {
        if !(1..=4).contains(&count)
            || self.storage.ids()?.len().saturating_add(count) > MAX_RECORDS
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Native turn journal is full",
            ));
        }
        continuation::select(self, input)
    }
    /// Only the fresh parent acceptance owner supplies this already-validated plan.
    pub(crate) fn accept_participant(
        &mut self,
        input: TurnInput,
        conversation_id: Uuid,
        timestamp: u64,
    ) -> io::Result<Admission> {
        let input = normalize(input)?;
        if self.storage.read(input.request_id)?.is_some() {
            return Err(invalid(
                "Participant native request identity already exists",
            ));
        }
        self.accept_selected(input, conversation_id, timestamp)
    }
    fn accept_selected(
        &mut self,
        input: TurnInput,
        conversation_id: Uuid,
        timestamp: u64,
    ) -> io::Result<Admission> {
        let record = TurnRecord {
            schema_version: 1,
            vault_id: self.storage.vault,
            conversation_id,
            input,
            phase: TurnPhase::Accepted,
            accepted_at_ms: timestamp,
            updated_at_ms: timestamp,
            output: None,
            failure_code: None,
        };
        let expected = self.storage.write(&record, None)?;
        let request_id = record.input.request_id;
        Ok(Admission::Fresh {
            record,
            claim: DispatchClaim {
                generation: self.storage.generation,
                request_id,
                expected,
            },
        })
    }
    pub fn claim(&mut self, claim: DispatchClaim) -> io::Result<RunningTurn> {
        if claim.generation != self.storage.generation {
            return Err(invalid("Native dispatch claim has retired"));
        }
        let (mut record, bytes) = self
            .storage
            .read(claim.request_id)?
            .ok_or_else(|| invalid("Native dispatch record is unavailable"))?;
        if bytes != claim.expected || record.phase != TurnPhase::Accepted {
            return Err(invalid("Native dispatch record changed"));
        }
        record.phase = TurnPhase::Running;
        record.updated_at_ms = record.updated_at_ms.max(now()?);
        let expected = self.storage.write(&record, Some(&claim.expected))?;
        Ok(RunningTurn {
            generation: claim.generation,
            record,
            expected,
        })
    }
    pub fn progress(
        &mut self,
        running: &mut RunningTurn,
        output: String,
    ) -> io::Result<TurnRecord> {
        if output.len() > MAX_OUTPUT_BYTES {
            return Err(invalid("Native turn response exceeds its bound"));
        }
        let mut record = running.record.clone();
        record.output = Some(output);
        self.publish(running, record)
    }
    pub fn finish(
        &mut self,
        running: &mut RunningTurn,
        outcome: TurnOutcome,
    ) -> io::Result<TurnRecord> {
        let mut record = running.record.clone();
        match outcome {
            TurnOutcome::Completed(output) => {
                record.phase = TurnPhase::Completed;
                record.output = Some(output);
            }
            TurnOutcome::Failed(reason) => {
                record.phase = TurnPhase::Failed;
                record.failure_code = Some(reason);
            }
            TurnOutcome::Cancelled => {
                record.phase = TurnPhase::Cancelled;
            }
            TurnOutcome::Unknown(reason) => {
                record.phase = TurnPhase::Unknown;
                record.failure_code = Some(reason);
            }
        }
        self.publish(running, record)
    }
    fn publish(
        &mut self,
        running: &mut RunningTurn,
        mut record: TurnRecord,
    ) -> io::Result<TurnRecord> {
        if running.generation != self.storage.generation
            || running.record.phase != TurnPhase::Running
        {
            return Err(invalid("Native running claim has retired"));
        }
        record.updated_at_ms = record.updated_at_ms.max(now()?);
        let expected = self.storage.write(&record, Some(&running.expected))?;
        running.record = record.clone();
        running.expected = expected;
        Ok(record)
    }
    pub fn read(&self, id: Uuid) -> io::Result<Option<TurnRecord>> {
        Ok(self.storage.read(id)?.map(|(record, _)| record))
    }
    pub fn list(&self) -> io::Result<Vec<TurnSummary>> {
        self.storage
            .ids()?
            .into_iter()
            .map(|id| {
                self.read(id)?
                    .map(TurnSummary::from)
                    .ok_or_else(|| invalid("Native turn vanished during query"))
            })
            .collect()
    }
    /// Native owner loss can retire authority, but never create another claim.
    pub(crate) fn interrupt(&mut self, id: Uuid, reason: &str) -> io::Result<TurnRecord> {
        let (mut record, bytes) = self
            .storage
            .read(id)?
            .ok_or_else(|| invalid("Native turn is unavailable"))?;
        record.phase = match record.phase {
            TurnPhase::Accepted => TurnPhase::Interrupted,
            TurnPhase::Running => TurnPhase::Unknown,
            _ => return Ok(record),
        };
        record.failure_code = Some(reason.into());
        record.updated_at_ms = record.updated_at_ms.max(now()?);
        self.storage.write(&record, Some(&bytes))?;
        Ok(record)
    }
}
