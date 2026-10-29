use super::super::{Host, Principal, Result};
use lapis_app_protocol::participants::{ParticipantCommand, ParticipantValue};
use uuid::Uuid;

impl Host {
    pub fn participant(
        &self,
        client: Uuid,
        principal: Principal,
        command: ParticipantCommand,
    ) -> Result<ParticipantValue> {
        self.client(client, principal)?
            .call_participant(command)
            .map_err(|error| error.0)
    }
}
