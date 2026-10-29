use super::{
    Arc, Data, Delivery, Job, Owner, QueuePhase, Result, admission, has_pending, invalid, receipt,
};
use crate::turns::Admission;
use lapis_app_protocol::turns::{TurnPhase, TurnRecord};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::watch;

pub(in crate::services::turns) fn drive(owner: &Arc<Owner>) {
    let next = start(owner);
    match next {
        Ok(Some((job, record, claim, selected))) => {
            super::super::worker::spawn(owner.clone(), job, record, claim, selected)
        }
        Ok(None) => {}
        Err(_) => {
            let paused = owner
                .data()
                .and_then(|mut d| d.queues.pause("dispatch_guard_failed"));
            if paused.is_err() {
                owner.failed.store(true, Ordering::Release);
                owner.changed.notify_all();
            }
        }
    }
}
type Next = (
    Arc<Job>,
    TurnRecord,
    crate::turns::DispatchClaim,
    super::super::launch::Selected,
);
fn start(owner: &Arc<Owner>) -> Result<Option<Next>> {
    let mut data = owner.data()?;
    let Some((id, work)) = data.queues.live.clone() else {
        return Ok(None);
    };
    if !data.jobs.is_empty() {
        return Ok(None);
    }
    if owner.require_enabled(&data).is_err() || work.cancellation_requested() {
        data.queues.pause("native_lane_stopped")?;
        return Ok(None);
    }
    let (mut queue, _) = data
        .queues
        .store
        .read(id)?
        .ok_or_else(super::super::failure)?;
    if queue.paused {
        data.queues.live = None;
        return Ok(None);
    }
    super::publication::flush(&work, &mut data, &mut queue)?;
    if !has_pending(&queue) {
        data.queues.live = None;
        return Ok(None);
    }
    let index = queue
        .entries
        .iter()
        .position(|e| e.phase == QueuePhase::Pending)
        .expect("pending entry");
    let entry = &queue.entries[index];
    let mut input = entry.input.clone();
    input.after_request_id = queue.head_request_id;
    let head = admission::head(&data, &input)?;
    if matches!(head.phase, TurnPhase::Accepted | TurnPhase::Running) {
        return Err(invalid("Native queue predecessor has no active owner"));
    }
    let target = admission::target(owner, &work, &input)?;
    if target != entry.target {
        return Err(invalid("Native queue target metadata changed"));
    }
    let native = admission::input(&input, entry.request_id, &target.default_mode)?;
    let mut selected = super::super::launch::select(owner, &work, &native)?;
    super::super::participants::acknowledge_history(
        &mut data,
        &owner.storage,
        &work,
        &mut selected.history,
    )?;
    admission::authored(&input, &selected)?;
    let delivery = Delivery {
        conversation_id: id,
        acceptance_id: entry.input.acceptance_id,
        capture: entry.input.capture.clone(),
    };
    let (_, expected) = data
        .queues
        .store
        .read(id)?
        .ok_or_else(super::super::failure)?;
    queue.entries[index].phase = QueuePhase::Started;
    let expected = data.queues.store.write(&queue, Some(&expected))?;
    // Persist the conservative started barrier before creating any native claim.
    let accepted = data.journal.accept(native);
    let (record, claim) = match accepted {
        Ok(Admission::Fresh { record, claim }) => (record, claim),
        _ => {
            let entry = &mut queue.entries[index];
            entry.phase = QueuePhase::Unknown;
            entry.receipt = Some(receipt(entry)?);
            queue.paused = true;
            queue.barrier = Some("native_admission_uncertain".into());
            data.queues.store.write(&queue, Some(&expected))?;
            data.queues.live = None;
            return Ok(None);
        }
    };
    queue.head_request_id = record.input.request_id;
    data.queues.store.write(&queue, Some(&expected))?;
    let job = Arc::new(Job {
        work,
        cancel: watch::channel(false).0,
        done: AtomicBool::new(false),
        dispatched: AtomicBool::new(false),
        session: Mutex::new(None),
        capture: Mutex::new(None),
        queued: Some(delivery),
        participant: None,
    });
    data.jobs.insert(record.input.request_id, job.clone());
    Ok(Some((job, record, claim, selected)))
}
/// Called under Data after durable turn publication and confirmed session cleanup.
pub(in crate::services::turns) fn complete(
    data: &mut Data,
    job: &Job,
    record: &TurnRecord,
    published: bool,
    terminal_receipt: Option<serde_json::Value>,
) -> Result<()> {
    if let Some(delivery) = &job.queued {
        let (mut queue, expected) = data
            .queues
            .store
            .read(delivery.conversation_id)?
            .ok_or_else(super::super::failure)?;
        let entry = queue
            .entries
            .iter_mut()
            .find(|e| e.input.acceptance_id == delivery.acceptance_id)
            .ok_or_else(super::super::failure)?;
        if entry.phase != QueuePhase::Started || entry.request_id != record.input.request_id {
            return Err(invalid("Native queue claim changed"));
        }
        entry.phase = phase(record);
        entry.receipt = Some(match terminal_receipt {
            Some(value) if value["outcome"] == entry.phase.outcome().expect("terminal") => value,
            _ => super::terminal_receipt(delivery, &record.phase)?,
        });
        entry.receipt_published = published;
        queue.head_request_id = record.input.request_id;
        data.queues.store.write(&queue, Some(&expected))?;
        if !has_pending(&queue)
            && queue.entries.iter().all(|e| e.receipt_published)
            && data
                .queues
                .live
                .as_ref()
                .is_some_and(|(id, _)| *id == queue.conversation_id)
        {
            data.queues.live = None;
        }
    }
    if record.phase != TurnPhase::Completed || *job.cancel.borrow() {
        data.queues.pause("native_turn_barrier")?;
    }
    Ok(())
}
fn phase(record: &TurnRecord) -> QueuePhase {
    match record.phase {
        TurnPhase::Completed => QueuePhase::Completed,
        TurnPhase::Cancelled => QueuePhase::Cancelled,
        TurnPhase::Failed => QueuePhase::Rejected,
        _ => QueuePhase::Unknown,
    }
}
