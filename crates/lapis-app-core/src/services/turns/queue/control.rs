use super::super::{Client, Error, Result};
use super::{Arc, QueuePhase, TurnQueue, admission, drive, has_pending, invalid, receipt};
use lapis_app_protocol::turns::TurnPhase;
use uuid::Uuid;

impl Client {
    pub fn turn_queue(&self, conversation_id: Uuid) -> Result<Option<TurnQueue>> {
        let (owner, _work) = self.turn_owner()?;
        Ok(owner
            .data()?
            .queues
            .store
            .read(conversation_id)?
            .map(|(q, _)| q))
    }
    /// Stop fences advancement before requesting cancellation of the active turn.
    pub fn pause_turn_queue(&self, conversation_id: Uuid) -> Result<TurnQueue> {
        let (owner, _work) = self.turn_owner()?;
        let mut data = owner.data()?;
        let (mut queue, bytes) = data
            .queues
            .store
            .read(conversation_id)?
            .ok_or_else(|| invalid("Native queue is unavailable"))?;
        queue.paused = true;
        queue.barrier = Some("explicit_stop".into());
        data.queues.store.write(&queue, Some(&bytes))?;
        if data
            .queues
            .live
            .as_ref()
            .is_some_and(|(id, _)| *id == conversation_id)
        {
            data.queues.live = None;
        }
        for (id, job) in &data.jobs {
            if data
                .journal
                .read(*id)?
                .is_some_and(|r| r.conversation_id == conversation_id)
            {
                job.cancel.send_replace(true);
            }
        }
        Ok(queue)
    }
    /// A cancellation only changes an entry that has never obtained a dispatch claim.
    pub fn cancel_queued_turn(
        &self,
        conversation_id: Uuid,
        acceptance_id: Uuid,
    ) -> Result<TurnQueue> {
        let (owner, work) = self.turn_owner()?;
        let mut data = owner.data()?;
        let (mut queue, bytes) = data
            .queues
            .store
            .read(conversation_id)?
            .ok_or_else(|| invalid("Native queue is unavailable"))?;
        let entry = queue
            .entries
            .iter_mut()
            .find(|e| e.input.acceptance_id == acceptance_id)
            .ok_or_else(|| invalid("Native queue acceptance is unavailable"))?;
        if entry.phase == QueuePhase::Started {
            return Err(Error::busy());
        }
        if entry.phase == QueuePhase::Pending {
            entry.phase = QueuePhase::Cancelled;
            entry.receipt = Some(receipt(entry)?);
            data.queues.store.write(&queue, Some(&bytes))?;
        }
        if data.jobs.is_empty() {
            super::publication::flush(&work, &mut data, &mut queue)?;
            if !has_pending(&queue)
                && data
                    .queues
                    .live
                    .as_ref()
                    .is_some_and(|(id, _)| *id == conversation_id)
            {
                data.queues.live = None;
            }
        }
        Ok(queue)
    }
    /// Explicitly acknowledge the current terminal native head; started entries never replay.
    pub fn resume_turn_queue(
        &self,
        conversation_id: Uuid,
        after_request_id: Uuid,
    ) -> Result<TurnQueue> {
        let (owner, work) = self.turn_owner()?;
        let mut data = owner.data()?;
        owner.require_enabled(&data)?;
        if !data.jobs.is_empty() || data.queues.live.is_some() {
            return Err(Error::busy());
        }
        let (mut queue, bytes) = data
            .queues
            .store
            .read(conversation_id)?
            .ok_or_else(|| invalid("Native queue is unavailable"))?;
        let mut input = queue.entries[0].input.clone();
        input.after_request_id = after_request_id;
        let head = admission::head(&data, &input)?;
        if matches!(head.phase, TurnPhase::Accepted | TurnPhase::Running) {
            return Err(Error::busy());
        }
        if queue.entries.iter().any(|e| e.phase == QueuePhase::Started) {
            return Err(Error::busy());
        }
        queue.head_request_id = after_request_id;
        queue.paused = false;
        queue.barrier = None;
        data.queues.store.write(&queue, Some(&bytes))?;
        if has_pending(&queue) {
            data.queues.live = Some((conversation_id, Arc::new(work)));
        } else {
            super::publication::flush(&work, &mut data, &mut queue)?;
        }
        drop(data);
        drive(&owner);
        self.turn_queue(conversation_id)?
            .ok_or_else(super::super::failure)
    }
}
