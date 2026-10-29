use super::{QueueInput, QueuePhase, TurnQueue};
use crate::services::{Client, Error, Result};
use lapis_app_protocol::{ErrorCode, queues as wire};
use uuid::Uuid;

impl Client {
    /// A transport has already admitted this client; accepted work outlives its reply.
    pub fn call_queue(&self, command: wire::QueueCommand) -> Result<wire::QueueValue> {
        let queue = match command {
            wire::QueueCommand::Accept { input } => Some(self.enqueue_turn(QueueInput {
                acceptance_id: input.acceptance_id,
                conversation_id: input.conversation_id,
                after_request_id: input.after_request_id,
                scope_dir: input.scope_dir,
                target_id: input.target_id,
                capture: input.capture,
            })?),
            wire::QueueCommand::Read { conversation_id } => {
                id(conversation_id)?;
                self.turn_queue(conversation_id)?
            }
            wire::QueueCommand::Stop { conversation_id } => {
                id(conversation_id)?;
                Some(self.pause_turn_queue(conversation_id)?)
            }
            wire::QueueCommand::Cancel {
                conversation_id,
                acceptance_id,
            } => {
                id(conversation_id)?;
                id(acceptance_id)?;
                Some(self.cancel_queued_turn(conversation_id, acceptance_id)?)
            }
            wire::QueueCommand::Resume {
                conversation_id,
                after_request_id,
            } => {
                id(conversation_id)?;
                id(after_request_id)?;
                Some(self.resume_turn_queue(conversation_id, after_request_id)?)
            }
        };
        Ok(wire::QueueValue::Queue {
            queue: queue.map(record).map(Box::new),
        })
    }
}
fn id(id: Uuid) -> Result<()> {
    if id.get_version_num() == 4 {
        Ok(())
    } else {
        Err(Error::new(
            ErrorCode::InvalidRequest,
            "Native queue identity is invalid",
        ))
    }
}
fn record(q: TurnQueue) -> wire::QueueRecord {
    wire::QueueRecord {
        schema_version: q.schema_version,
        vault_id: q.vault_id,
        conversation_id: q.conversation_id,
        scope_dir: q.scope_dir,
        head_request_id: q.head_request_id,
        paused: q.paused,
        barrier: q.barrier,
        entries: q
            .entries
            .into_iter()
            .map(|e| wire::QueueEntry {
                input: wire::QueueAccept {
                    acceptance_id: e.input.acceptance_id,
                    conversation_id: e.input.conversation_id,
                    after_request_id: e.input.after_request_id,
                    scope_dir: e.input.scope_dir,
                    target_id: e.input.target_id,
                    capture: e.input.capture,
                },
                request_id: e.request_id,
                phase: match e.phase {
                    QueuePhase::Pending => wire::QueuePhase::Pending,
                    QueuePhase::Started => wire::QueuePhase::Started,
                    QueuePhase::Completed => wire::QueuePhase::Completed,
                    QueuePhase::Cancelled => wire::QueuePhase::Cancelled,
                    QueuePhase::Rejected => wire::QueuePhase::Rejected,
                    QueuePhase::Unknown => wire::QueuePhase::Unknown,
                },
                receipt: e.receipt,
                receipt_published: e.receipt_published,
            })
            .collect(),
    }
}
