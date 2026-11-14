pub(in crate::host) mod storage;
use super::{Host, Principal, Result};
use lapis_app_protocol::native_tasks::{NativeTaskCommand, NativeTaskValue};
use uuid::Uuid;

impl Host {
    /// Installation startup restores admitted domain declarations before listening.
    pub async fn restore_task_declarations(&self) -> std::io::Result<()> {
        for vault in self
            .vaults()
            .map_err(|e| std::io::Error::other(e.message))?
        {
            if vault.archived_at.is_some() {
                continue;
            }
            let owner = self
                .runtime
                .get(vault.vault_id)
                .map_err(std::io::Error::other)?;
            let client = owner.attach().map_err(std::io::Error::other)?;
            let value = client
                .control_tasks(NativeTaskCommand::Status {})
                .await
                .map_err(std::io::Error::other)?;
            if let NativeTaskValue::Status { status } = value
                && let Some(configuration) = status.configuration
            {
                client
                    .control_tasks(NativeTaskCommand::Configure { configuration })
                    .await
                    .map_err(std::io::Error::other)?;
            }
        }
        Ok(())
    }
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
