use super::{ParticipantDelivery, ParticipantMessage, ParticipantMessageInput, ParticipantState};
use crate::services::{Client, Result};
use lapis_app_protocol::participants as wire;

impl Client {
    /// Admission and lifetime remain native; a reply never owns accepted work.
    pub fn call_participant(
        &self,
        command: wire::ParticipantCommand,
    ) -> Result<wire::ParticipantValue> {
        match command {
            wire::ParticipantCommand::Profiles {} => Ok(wire::ParticipantValue::Profiles {
                profiles: self.participant_profiles()?,
            }),
            wire::ParticipantCommand::ForTurn { request_id } => {
                Ok(value(self.participant_for_turn(request_id)?))
            }
            wire::ParticipantCommand::Accept { input } => Ok(value(Some(
                self.submit_participant_message(ParticipantMessageInput {
                    acceptance_id: input.acceptance_id,
                    user: input.user,
                    dispatch: input.dispatch,
                    turns: input.turns,
                    profiles: input.profiles,
                })?,
            ))),
            wire::ParticipantCommand::Read { acceptance_id } => {
                Ok(value(self.participant_message(acceptance_id)?))
            }
            wire::ParticipantCommand::Stop { acceptance_id } => {
                Ok(wire::ParticipantValue::StopRequested {
                    accepted: self.control_participant(acceptance_id, None)?,
                })
            }
            wire::ParticipantCommand::Cancel {
                acceptance_id,
                request_id,
            } => Ok(wire::ParticipantValue::CancelRequested {
                accepted: self.control_participant(acceptance_id, Some(request_id))?,
            }),
        }
    }
}
fn value(message: Option<ParticipantMessage>) -> wire::ParticipantValue {
    wire::ParticipantValue::Message {
        message: message.map(|message| {
            Box::new(wire::ParticipantMessage {
                schema_version: message.schema_version,
                vault_id: message.vault_id,
                acceptance_id: message.acceptance_id,
                conversation_id: message.conversation_id,
                input: wire::ParticipantMessageInput {
                    acceptance_id: message.input.acceptance_id,
                    user: message.input.user,
                    dispatch: message.input.dispatch,
                    turns: message.input.turns,
                    profiles: message.input.profiles,
                },
                deliveries: message.deliveries.into_iter().map(delivery).collect(),
                recovered: message.recovered,
            })
        }),
    }
}
fn delivery(delivery: ParticipantDelivery) -> wire::ParticipantDelivery {
    wire::ParticipantDelivery {
        delivery_id: delivery.delivery_id,
        request_id: delivery.request_id,
        agent_binding_id: delivery.agent_binding_id,
        session_id: delivery.session_id,
        state: match delivery.state {
            ParticipantState::Pending => wire::ParticipantState::Pending,
            ParticipantState::Started => wire::ParticipantState::Started,
            ParticipantState::Completed => wire::ParticipantState::Completed,
            ParticipantState::Rejected => wire::ParticipantState::Rejected,
            ParticipantState::Unknown => wire::ParticipantState::Unknown,
            ParticipantState::NotSent => wire::ParticipantState::NotSent,
            ParticipantState::Cancelled => wire::ParticipantState::Cancelled,
        },
        outcome_published: delivery.outcome_published,
    }
}
