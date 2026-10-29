pub(in crate::host) mod storage;
use super::{Host, Principal, Result};
use lapis_app_protocol::native_tasks::{NativeTaskCommand, NativeTaskValue};
use uuid::Uuid;

impl Host {
    pub async fn tasks(
        &self,
        client: Uuid,
        principal: Principal,
        command: NativeTaskCommand,
    ) -> Result<NativeTaskValue> {
        self.client(client, principal)?
            .control_tasks(command)
            .await
            .map_err(|error| error.0)
    }
}
