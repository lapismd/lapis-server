use super::{Host, Principal, Result};
use lapis_app_protocol::repository_restore::{RepositoryRestoreCommand, RepositoryRestoreValue};
use uuid::Uuid;
#[cfg(test)]
mod tests;

impl Host {
    pub async fn repository_restore(
        &self,
        client: Uuid,
        principal: Principal,
        command: RepositoryRestoreCommand,
    ) -> Result<RepositoryRestoreValue> {
        let result = self
            .client(client, principal)?
            .control_repository_restore(command)
            .await;
        // Admission owns accepted native effects; retirement fences delivery.
        self.client(client, principal)?;
        result.map_err(|error| error.0)
    }
}
