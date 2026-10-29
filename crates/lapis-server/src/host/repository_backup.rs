use super::{Host, Principal, Result};
use lapis_app_protocol::repository_backup::{RepositoryBackupCommand, RepositoryBackupSnapshot};
use uuid::Uuid;
#[cfg(test)]
mod tests;

impl Host {
    pub async fn repository_backup(
        &self,
        client: Uuid,
        principal: Principal,
        command: RepositoryBackupCommand,
    ) -> Result<RepositoryBackupSnapshot> {
        let result = self
            .client(client, principal)?
            .control_repository_backup(command)
            .await;
        // Accepted work remains native-owned; retirement fences reply delivery.
        self.client(client, principal)?;
        result.map_err(|error| error.0)
    }
}
