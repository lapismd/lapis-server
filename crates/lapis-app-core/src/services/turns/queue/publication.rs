use super::{Data, Result, TurnQueue, Work};
use lapis_app_protocol::{
    ExpectedVersion,
    turns::{TurnContinuation, TurnInput},
};
use serde_json::Value;

/// Only called while the native execution lane is idle and Data is locked.
pub(super) fn flush(work: &Work, data: &mut Data, queue: &mut TurnQueue) -> Result<()> {
    if !data.jobs.is_empty() {
        return Ok(());
    }
    let pending = queue
        .entries
        .iter()
        .filter(|e| e.receipt.is_some() && !e.receipt_published)
        .map(|e| e.receipt.clone().expect("terminal receipt"))
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(());
    }
    let (_, expected) = data
        .queues
        .store
        .read(queue.conversation_id)?
        .ok_or_else(super::super::failure)?;
    let head = data
        .journal
        .read(queue.head_request_id)?
        .ok_or_else(super::super::failure)?;
    let input = TurnInput {
        continuation: Some(TurnContinuation {
            conversation_id: queue.conversation_id,
            after_request_id: queue.head_request_id,
        }),
        ..head.input
    };
    let history = super::super::history::read(work, &input)?.ok_or_else(super::super::failure)?;
    let mut text = history.transcript.text;
    for receipt in pending {
        let existing = text
            .lines()
            .filter_map(|s| serde_json::from_str::<Value>(s).ok())
            .find(|v| v["id"] == receipt["id"]);
        if let Some(existing) = existing {
            // A cold started barrier may reconstruct the same immutable outcome
            // after its portable publication but before the private acknowledgement.
            if existing["queued"] != receipt["queued"]
                || existing["outcome"] != receipt["outcome"]
                || existing["parentId"] != receipt["parentId"]
            {
                return Err(super::invalid("Native queue receipt changed"));
            }
            if let Some(entry) = queue.entries.iter_mut().find(|e| {
                e.receipt
                    .as_ref()
                    .is_some_and(|v| v["id"] == existing["id"])
            }) {
                entry.receipt = Some(existing);
            }
        } else {
            text.push_str(&format!("{receipt}\n"));
        }
    }
    work.write_text(
        &history.transcript.path,
        &text,
        &ExpectedVersion::Version {
            version: history.transcript.version,
        },
    )?;
    for entry in &mut queue.entries {
        if entry.receipt.is_some() {
            entry.receipt_published = true;
        }
    }
    data.queues.store.write(queue, Some(&expected))?;
    Ok(())
}
