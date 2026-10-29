use super::super::{Client, Error, Job, Result, conversation::Conversation, launch, worker};
use super::{
    Delivery, ParticipantDelivery, ParticipantMessage, ParticipantMessageInput, ParticipantState,
    invalid, store, validation,
};
use crate::turns::Admission;
use serde_json::Value;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8},
};
use tokio::sync::watch;
use uuid::Uuid;

impl Client {
    /// Only a fresh durable parent acceptance can create its bounded native claims.
    pub fn submit_participant_message(
        &self,
        input: ParticipantMessageInput,
    ) -> Result<ParticipantMessage> {
        validation::input(&input)?;
        let (owner, work) = self.turn_owner()?;
        let mut data = owner.data()?;
        store::open(&mut data, &owner.storage, &work)?;
        if let Some((message, _)) = data
            .participants
            .as_ref()
            .expect("opened")
            .store
            .read(input.acceptance_id)?
        {
            return if message.input == input {
                Ok(message)
            } else {
                Err(invalid(
                    "Participant acceptance was reused with different input",
                ))
            };
        }
        owner.require_enabled(&data)?;
        if !data.jobs.is_empty() || data.queues.blocks(&input.turns[0])? {
            return Err(Error::busy());
        }
        if data
            .participants
            .as_ref()
            .expect("opened")
            .store
            .ids()?
            .len()
            >= 256
        {
            return Err(Error::busy());
        }
        for turn in &input.turns {
            if data.journal.read(turn.request_id)?.is_some() {
                return Err(invalid(
                    "Participant native request identity already exists",
                ));
            }
        }
        super::profiles::admit(&owner, &work, &input)?;
        let (conversation_id, time) = data
            .journal
            .plan_participants(&input.turns[0], input.turns.len())?;
        let mut selected = input
            .turns
            .iter()
            .map(|turn| launch::select(&owner, &work, turn))
            .collect::<Result<Vec<_>>>()?;
        super::acknowledge_history(&mut data, &owner.storage, &work, &mut selected[0].history)?;
        if let Some(history) = &selected[0].history {
            authored(&input, &history.transcript.text)?;
        }
        let deliveries = input
            .turns
            .iter()
            .enumerate()
            .map(|(index, turn)| ParticipantDelivery {
                delivery_id: input.dispatch["deliveries"][index]["id"]
                    .as_str()
                    .expect("validated id")
                    .into(),
                request_id: turn.request_id,
                agent_binding_id: Uuid::new_v4(),
                session_id: Uuid::new_v4(),
                state: ParticipantState::Pending,
                outcome_published: false,
            })
            .collect();
        let mut message = ParticipantMessage {
            schema_version: 1,
            vault_id: work.vault.id,
            acceptance_id: input.acceptance_id,
            conversation_id,
            input,
            deliveries,
            recovered: false,
        };
        let expected = data
            .participants
            .as_ref()
            .expect("opened")
            .store
            .write(&message, None)?;
        let mut claims = Vec::new();
        let prepared = (|| -> Result<_> {
            for (index, turn) in message.input.turns.iter().enumerate() {
                match data.journal.accept_participant(
                    turn.clone(),
                    conversation_id,
                    time.checked_add(index as u64)
                        .ok_or_else(|| invalid("Participant timestamp exceeds its limit"))?,
                )? {
                    Admission::Fresh { record, claim } => claims.push((record, claim)),
                    _ => return Err(invalid("Participant dispatch claim is not fresh")),
                }
            }
            Conversation::participants(&work, &claims[0].0, selected[0].history.take(), &message)
        })();
        let conversations = match prepared {
            Ok(conversations) => conversations,
            Err(error) => {
                for (record, _) in &claims {
                    data.journal
                        .interrupt(record.input.request_id, "participant_preparation_failed")?;
                }
                for delivery in &mut message.deliveries {
                    delivery.state = ParticipantState::NotSent;
                }
                data.participants
                    .as_ref()
                    .expect("opened")
                    .store
                    .write(&message, Some(&expected))?;
                return Err(error);
            }
        };
        let work = Arc::new(work);
        let mut jobs = Vec::new();
        for (index, ((record, claim), (selected, conversation))) in claims
            .into_iter()
            .zip(selected.into_iter().zip(conversations))
            .enumerate()
        {
            let job = Arc::new(Job {
                queued: None,
                participant: Some(Delivery {
                    message: message.acceptance_id,
                    index,
                    session_id: message.deliveries[index].session_id,
                    conversation,
                    skipped: AtomicU8::new(0),
                }),
                work: work.clone(),
                cancel: watch::channel(false).0,
                done: AtomicBool::new(false),
                dispatched: AtomicBool::new(false),
                session: Mutex::new(None),
                capture: Mutex::new(None),
            });
            data.jobs.insert(record.input.request_id, job.clone());
            jobs.push((job, record, claim, selected));
        }
        drop(data);
        for (job, record, claim, selected) in jobs {
            worker::spawn(owner.clone(), job, record, claim, selected);
        }
        Ok(message)
    }
    pub fn participant_message(&self, acceptance_id: Uuid) -> Result<Option<ParticipantMessage>> {
        if acceptance_id.get_version_num() != 4 {
            return Err(invalid("Participant acceptance identity is invalid"));
        }
        let (owner, work) = self.turn_owner()?;
        let mut data = owner.data()?;
        store::open(&mut data, &owner.storage, &work)?;
        Ok(data
            .participants
            .as_ref()
            .expect("opened")
            .store
            .read(acceptance_id)?
            .map(|(message, _)| message))
    }
}
fn authored(input: &ParticipantMessageInput, text: &str) -> Result<()> {
    for line in text.lines() {
        let value: Value = serde_json::from_str(line)
            .map_err(|_| invalid("Native participant history changed"))?;
        if value["id"] == input.user["id"]
            || value["type"] == "agent.delivery"
                && (value["dispatchId"] == input.dispatch["id"]
                    || input.dispatch["deliveries"]
                        .as_array()
                        .expect("validated deliveries")
                        .iter()
                        .any(|d| d["id"] == value["delivery"]["id"]))
        {
            return Err(invalid(
                "Participant authored, dispatch or delivery identity already exists",
            ));
        }
    }
    Ok(())
}
