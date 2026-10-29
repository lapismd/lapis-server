use super::super::{Host, Principal, Result};
use lapis_app_protocol::queues::{QueueCommand, QueueValue};
use uuid::Uuid;
impl Host {
    pub fn queue(
        &self,
        client: Uuid,
        principal: Principal,
        command: QueueCommand,
    ) -> Result<QueueValue> {
        self.client(client, principal)?
            .call_queue(command)
            .map_err(|error| error.0)
    }
}
