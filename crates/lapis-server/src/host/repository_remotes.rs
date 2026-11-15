use super::{Host, Principal, Result};
use lapis_app_protocol::repository_remotes::{RepositoryRemotesCommand, RepositoryRemotesSnapshot};
use lapis_app_protocol::repository_sync::{RepositorySyncCommand, RepositorySyncSnapshot};
use uuid::Uuid;

impl Host {
    pub async fn repository_synchronization(
        &self,
        client: Uuid,
        principal: Principal,
        vault: Uuid,
        generation: Uuid,
        command: Option<RepositorySyncCommand>,
    ) -> Result<RepositorySyncSnapshot> {
        let owner = self.observation_client(client, principal, vault, generation)?;
        let result = match command {
            Some(command) => owner.control_repository_synchronization(command).await,
            None => owner.repository_synchronization(),
        };
        self.observation_client(client, principal, vault, generation)?;
        result.map_err(|error| error.0)
    }
    pub async fn repository_remotes(
        &self,
        client: Uuid,
        principal: Principal,
        vault: Uuid,
        generation: Uuid,
        command: Option<RepositoryRemotesCommand>,
    ) -> Result<RepositoryRemotesSnapshot> {
        let owner = self.observation_client(client, principal, vault, generation)?;
        let result = match command {
            Some(command) => owner.control_repository_remotes(command).await,
            None => owner.repository_remote_settings().await,
        };
        // Accepted native work survives response cancellation; retirement fences delivery.
        self.observation_client(client, principal, vault, generation)?;
        result.map_err(|error| error.0)
    }

    pub async fn repository_working_copy(
        &self,
        client: Uuid,
        principal: Principal,
        vault: Uuid,
        generation: Uuid,
    ) -> Result<String> {
        let owner = self.observation_client(client, principal, vault, generation)?;
        let result = owner.repository_working_copy().await;
        self.observation_client(client, principal, vault, generation)?;
        result.map_err(|error| error.0)
    }
}
