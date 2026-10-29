use super::{Host, Principal, Result};
use lapis_app_protocol::history_index::{HistoryIndexCommand, HistoryIndexValue};
use uuid::Uuid;

impl Host {
    pub async fn history_index(
        &self,
        client: Uuid,
        principal: Principal,
        command: HistoryIndexCommand,
    ) -> Result<HistoryIndexValue> {
        self.client(client, principal)?
            .control_history_index(command)
            .await
            .map_err(|error| error.0)
    }
}
