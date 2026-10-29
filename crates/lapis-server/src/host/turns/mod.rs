mod participants;
mod policy;
mod queues;
pub(super) mod storage;
use super::{Host, Principal, Result};
use lapis_app_protocol::turns::{TurnCommand, TurnValue};
pub use policy::AgentConfig;
use uuid::Uuid;

impl Host {
    pub fn turn(
        &self,
        client: Uuid,
        principal: Principal,
        command: TurnCommand,
    ) -> Result<TurnValue> {
        self.client(client, principal)?
            .call_turn(command)
            .map_err(|error| error.0)
    }
}
