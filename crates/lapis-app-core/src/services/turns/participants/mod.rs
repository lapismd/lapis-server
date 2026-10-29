mod acknowledge;
mod admission;
mod control;
mod lookup;
mod profiles;
mod store;
mod types;
mod validation;
mod wire;
use super::{Data, Error, Job, Result, conversation::Conversation};
pub(super) use acknowledge::history as acknowledge_history;
use lapis_app_protocol::{ErrorCode, turns::TurnRecord};
use std::sync::atomic::{AtomicU8, Ordering};
pub(super) use store::Messages;
pub use types::{
    ParticipantDelivery, ParticipantMessage, ParticipantMessageInput, ParticipantState,
};
use uuid::Uuid;

pub(super) fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::Unsupported, message)
}
pub(super) struct Delivery {
    pub message: Uuid,
    pub index: usize,
    pub session_id: Uuid,
    pub conversation: Conversation,
    pub skipped: AtomicU8,
}
impl Delivery {
    pub fn skip(&self, state: ParticipantState) {
        let code = if state == ParticipantState::Cancelled {
            2
        } else {
            1
        };
        self.skipped.store(code, Ordering::Release);
    }
    pub fn skipped(&self) -> Option<ParticipantState> {
        match self.skipped.load(Ordering::Acquire) {
            1 => Some(ParticipantState::NotSent),
            2 => Some(ParticipantState::Cancelled),
            _ => None,
        }
    }
}
pub(super) fn started(
    data: &mut Data,
    job: &Job,
    record: &TurnRecord,
) -> Result<Option<ParticipantState>> {
    let Some(delivery) = &job.participant else {
        return Ok(None);
    };
    let store = &data.participants.as_ref().ok_or_else(super::failure)?.store;
    let (mut message, expected) = store.read(delivery.message)?.ok_or_else(super::failure)?;
    let entry = message
        .deliveries
        .get(delivery.index)
        .ok_or_else(super::failure)?;
    if entry.state != ParticipantState::Pending
        || entry.request_id != record.input.request_id
        || message.conversation_id != record.conversation_id
        || message.input.turns[delivery.index] != record.input
    {
        return Err(invalid("Participant dispatch authority changed"));
    }
    if *job.cancel.borrow() {
        return Ok(Some(ParticipantState::Cancelled));
    }
    if message.deliveries.iter().any(|d| {
        matches!(
            d.state,
            ParticipantState::Rejected | ParticipantState::Unknown
        )
    }) {
        return Ok(Some(ParticipantState::NotSent));
    }
    message.deliveries[delivery.index].state = ParticipantState::Started;
    store.write(&message, Some(&expected))?;
    Ok(None)
}
pub(super) fn complete(
    data: &mut Data,
    job: &Job,
    record: &TurnRecord,
    published: bool,
) -> Result<()> {
    let Some(delivery) = &job.participant else {
        return Ok(());
    };
    let store = &data.participants.as_ref().ok_or_else(super::failure)?.store;
    let (mut message, expected) = store.read(delivery.message)?.ok_or_else(super::failure)?;
    let entry = message
        .deliveries
        .get_mut(delivery.index)
        .ok_or_else(super::failure)?;
    if entry.request_id != record.input.request_id
        || message.input.turns[delivery.index] != record.input
        || message.conversation_id != record.conversation_id
    {
        return Err(invalid("Participant completion authority changed"));
    }
    let state = delivery
        .skipped()
        .unwrap_or_else(|| ParticipantState::outcome(record.phase));
    if entry.state.terminal() {
        return if entry.state == state && entry.outcome_published == published {
            Ok(())
        } else {
            Err(invalid("Participant outcome cannot be overwritten"))
        };
    }
    entry.state = state;
    entry.outcome_published = published;
    store.write(&message, Some(&expected))?;
    Ok(())
}

pub(super) fn validate_history_delivery(value: &serde_json::Value) -> Result<()> {
    crate::turns::participant_capture::validate_delivery(value["delivery"].clone())
        .map_err(|e| invalid(&e.message))?;
    for key in ["parentId", "dispatchId"] {
        crate::turns::queue_capture::fields::string(value, key, "Agent delivery")
            .map_err(|e| invalid(&e.message))?;
    }
    Ok(())
}
