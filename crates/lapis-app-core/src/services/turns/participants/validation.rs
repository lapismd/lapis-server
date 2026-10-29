use super::{ParticipantMessage, ParticipantMessageInput, ParticipantState, invalid};
use crate::{
    record_store::StoredRecord,
    services::Result,
    turns::{participant_capture, queue_capture},
};
use serde_json::Value;
use std::{collections::HashSet, io};
use uuid::Uuid;

pub(super) fn input(input: &ParticipantMessageInput) -> Result<()> {
    participant_capture::validate_dispatch(input.dispatch.clone())
        .map_err(|e| invalid(&e.message))?;
    let recipients = queue_capture::validate_user(&input.user).map_err(|e| invalid(&e.message))?;
    let deliveries = input.dispatch["deliveries"]
        .as_array()
        .expect("validated deliveries");
    if input.acceptance_id.get_version_num() != 4
        || recipients.is_empty()
        || input.turns.len() != deliveries.len()
        || recipients.len() != deliveries.len()
        || input.dispatch["userEntryId"] != input.user["id"]
        || deliveries
            .iter()
            .any(|d| d["state"] != "pending" || d.get("agentBindingId").is_some())
        || serde_json::to_vec(input)
            .map_err(|_| invalid("Invalid participant message"))?
            .len()
            > 2 * 1024 * 1024
    {
        return Err(invalid(
            "A native participant message requires its exact unstarted capture",
        ));
    }
    if input.user.get("contextBlocks").is_some() || input.turns.iter().any(|t| t.context.is_some())
    {
        return Err(super::unsupported(
            "Native participants support plain text without rich context",
        ));
    }
    super::profiles::captured(input)?;
    let first = &input.turns[0];
    let mut native_ids = HashSet::from([input.acceptance_id]);
    for (index, turn) in input.turns.iter().enumerate() {
        if crate::turns::normalize(turn.clone())? != *turn
            || !native_ids.insert(turn.request_id)
            || turn.scope_dir != first.scope_dir
            || turn.continuation != first.continuation
            || Value::String(turn.prompt.clone()) != input.user["text"]
            || deliveries[index]["recipient"] != input.user["recipients"][index]
        {
            return Err(invalid(
                "Native participant inputs do not match their authored parent",
            ));
        }
        let delivery_id = deliveries[index]["id"].as_str().expect("validated id");
        if participant_capture::STATES
            .iter()
            .any(|state| input.user["id"] == format!("delivery:{delivery_id}:{state}"))
        {
            return Err(invalid(
                "Authored participant identity aliases a delivery outcome",
            ));
        }
    }
    Ok(())
}
impl StoredRecord for ParticipantMessage {
    const KIND: &'static str = "participant-message";
    const MAX_RECORD_BYTES: usize = 8 * 1024 * 1024;
    const MAX_RECORDS: usize = 256;
    fn id(&self) -> Uuid {
        self.acceptance_id
    }
    fn validate(&self, vault: Uuid, id: Uuid) -> io::Result<()> {
        let bad = || {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid native participant record",
            )
        };
        input(&self.input).map_err(|_| bad())?;
        if self.schema_version != 1
            || self.vault_id != vault
            || self.acceptance_id != id
            || self.input.acceptance_id != id
            || self.conversation_id.get_version_num() != 4
            || self.deliveries.len() != self.input.turns.len()
            || self.input.turns[0]
                .continuation
                .as_ref()
                .is_some_and(|c| c.conversation_id != self.conversation_id)
        {
            return Err(bad());
        }
        let mut ids = HashSet::from([id]);
        for (index, delivery) in self.deliveries.iter().enumerate() {
            if delivery.request_id != self.input.turns[index].request_id
                || delivery.delivery_id.as_str()
                    != self.input.dispatch["deliveries"][index]["id"]
                        .as_str()
                        .expect("validated id")
                || [
                    delivery.request_id,
                    delivery.agent_binding_id,
                    delivery.session_id,
                ]
                .iter()
                .any(|id| id.get_version_num() != 4 || !ids.insert(*id))
                || matches!(
                    delivery.state,
                    ParticipantState::Pending | ParticipantState::Started
                ) && delivery.outcome_published
                || delivery.state == ParticipantState::Completed && !delivery.outcome_published
            {
                return Err(bad());
            }
        }
        Ok(())
    }
}
