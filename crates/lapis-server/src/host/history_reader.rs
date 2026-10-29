use super::{Host, Principal};
use lapis_app_core::services::{HistoryError, HistoryMethod};
use uuid::Uuid;

impl Host {
    pub async fn read_history(
        &self,
        client: Uuid,
        principal: Principal,
        method: HistoryMethod,
        frame: String,
    ) -> Result<String, HistoryError> {
        let owner = self
            .client(client, principal)
            .map_err(|_| HistoryError::Denied)?;
        let result = owner.read_history(method, frame).await?;
        self.client(client, principal)
            .map_err(|_| HistoryError::Denied)?;
        Ok(result)
    }
}
