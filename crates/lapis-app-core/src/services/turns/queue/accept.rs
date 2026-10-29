use super::super::{Client, Error, Result};
use super::{
    Arc, QueueEntry, QueueInput, QueuePhase, TurnQueue, admission, drive, has_pending, invalid,
    profile,
};
use lapis_app_protocol::{ErrorCode, turns::TurnPhase};
use serde_json::Value;
use uuid::Uuid;

impl Client {
    /// Durable source-only acceptance, independent of client response lifetime.
    pub fn enqueue_turn(&self, input: QueueInput) -> Result<TurnQueue> {
        profile(&input)?;
        let (owner, work) = self.turn_owner()?;
        let mut data = owner.data()?;
        // A lost reply remains queryable even after pause, disable or target removal.
        for id in data.queues.store.ids()? {
            let (queue, _) = data
                .queues
                .store
                .read(id)?
                .ok_or_else(super::super::failure)?;
            if let Some(entry) = queue
                .entries
                .iter()
                .find(|e| e.input.acceptance_id == input.acceptance_id)
            {
                return if entry.input == input {
                    Ok(queue)
                } else {
                    Err(invalid(
                        "Queue acceptance identity was reused with different input",
                    ))
                };
            }
        }
        owner.require_enabled(&data)?;
        if data
            .queues
            .live
            .as_ref()
            .is_some_and(|(id, _)| *id != input.conversation_id)
        {
            return Err(Error::busy());
        }
        let head = admission::head(&data, &input)?;
        if !matches!(
            head.phase,
            TurnPhase::Accepted | TurnPhase::Running | TurnPhase::Completed
        ) {
            return Err(invalid(
                "Native queue predecessor requires explicit recovery",
            ));
        }
        let target = admission::target(&owner, &work, &input)?;
        let native = admission::input(&input, Uuid::new_v4(), &target.default_mode)?;
        let selected = super::super::launch::select(&owner, &work, &native)?;
        admission::authored(&input, &selected)?;
        let saved = data.queues.store.read(input.conversation_id)?;
        let (mut queue, bytes) = match saved {
            Some((queue, bytes)) => (queue, Some(bytes)),
            None => {
                if data.queues.store.ids()?.len() >= 256 {
                    return Err(Error::busy());
                }
                (
                    TurnQueue {
                        schema_version: 1,
                        vault_id: work.vault.id,
                        conversation_id: input.conversation_id,
                        scope_dir: input.scope_dir.clone(),
                        head_request_id: input.after_request_id,
                        paused: false,
                        barrier: None,
                        entries: Vec::new(),
                    },
                    None,
                )
            }
        };
        if queue.scope_dir != input.scope_dir
            || queue.head_request_id != input.after_request_id
            || queue.entries.len() >= 128
            || (queue.paused && has_pending(&queue))
        {
            return Err(Error::busy());
        }
        if queue.entries.iter().any(|e| {
            e.input.capture["id"] == input.capture["id"]
                || e.input.capture["user"]["id"] == input.capture["user"]["id"]
        }) {
            return Err(invalid("Queued authored identities must remain distinct"));
        }
        let mut pending = queue
            .entries
            .iter()
            .filter(|e| matches!(e.phase, QueuePhase::Pending | QueuePhase::Started))
            .map(|e| e.input.capture.clone())
            .collect::<Vec<_>>();
        pending.push(input.capture.clone());
        crate::turns::queue_capture::validate_queue(Some(Value::Array(pending)))
            .map_err(|_| Error::new(ErrorCode::Busy, "Native pending queue is full"))?;
        queue.entries.push(QueueEntry {
            input,
            request_id: native.request_id,
            phase: QueuePhase::Pending,
            receipt: None,
            receipt_published: false,
            target,
        });
        queue.paused = false;
        queue.barrier = None;
        data.queues.store.write(&queue, bytes.as_deref())?;
        data.queues.live = Some((queue.conversation_id, Arc::new(work)));
        drop(data);
        drive(&owner);
        self.turn_queue(queue.conversation_id)?
            .ok_or_else(super::super::failure)
    }
}
