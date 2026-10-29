mod accept;
mod admission;
mod control;
mod dispatch;
mod publication;
mod store;
mod types;
mod wire;
use super::{Data, Job, Owner, Result, Work};
pub(super) use dispatch::{complete, drive};
use std::sync::Arc;
pub(super) use store::Queues;
pub use types::{QueueEntry, QueueInput, QueuePhase, TurnQueue};
use types::{Target, invalid, profile, receipt};

#[derive(Clone)]
pub(super) struct Delivery {
    pub conversation_id: uuid::Uuid,
    pub acceptance_id: uuid::Uuid,
    pub capture: serde_json::Value,
}
pub(super) fn has_pending(queue: &TurnQueue) -> bool {
    queue.entries.iter().any(|e| e.phase == QueuePhase::Pending)
}
pub(super) fn terminal_receipt(
    delivery: &Delivery,
    phase: &lapis_app_protocol::turns::TurnPhase,
) -> Result<serde_json::Value> {
    use lapis_app_protocol::turns::TurnPhase;
    let outcome = match phase {
        TurnPhase::Completed => "completed",
        TurnPhase::Cancelled => "cancelled",
        TurnPhase::Failed => "rejected",
        _ => "unknown",
    };
    let capture = crate::turns::queue_capture::validate_message(delivery.capture.clone())
        .map_err(|e| invalid(&e.message))?;
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    crate::turns::queue_capture::create_receipt(&capture, outcome, &now)
        .map(crate::turns::queue_capture::QueueReceipt::into_value)
        .map_err(|e| invalid(&e.message))
}
