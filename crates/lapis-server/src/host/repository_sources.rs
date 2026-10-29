use super::{Host, Principal};
use lapis_app_core::services::HistoryError;
use lapis_app_protocol::repository_sources::{RepositorySources, RepositorySourcesCommand};
use uuid::Uuid;

impl Host {
    pub async fn repository_sources(
        &self,
        client: Uuid,
        principal: Principal,
        command: RepositorySourcesCommand,
    ) -> Result<RepositorySources, HistoryError> {
        let result = self
            .client(client, principal)
            .map_err(|_| HistoryError::Denied)?
            .repository_sources(command)
            .await;
        self.client(client, principal)
            .map_err(|_| HistoryError::Denied)?;
        result
    }
}
