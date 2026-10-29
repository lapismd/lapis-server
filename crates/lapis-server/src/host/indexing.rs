use super::{Host, Principal, Result};
use lapis_app_protocol::indexing::{IndexingCommand, IndexingValue};
use uuid::Uuid;

impl Host {
    pub async fn indexing(
        &self,
        client: Uuid,
        principal: Principal,
        command: IndexingCommand,
    ) -> Result<IndexingValue> {
        self.client(client, principal)?
            .control_indexing(command)
            .await
            .map_err(|error| error.0)
    }
}
