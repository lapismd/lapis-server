use super::{Host, Principal, Result};
use uuid::Uuid;

impl Host {
    /// The configured application profile selects automatic backup. Explicit
    /// file-only profiles retain their existing unavailable-service boundary.
    pub async fn prepare_client(&self, client: Uuid, principal: Principal) -> Result<()> {
        let client = self.client(client, principal)?;
        if self.automatic_backup
            && let Err(error) = client.start_backup().await
        {
            if error.code() != lapis_app_protocol::ErrorCode::Io {
                return Err(error.0);
            }
            // Managed capture is optional. Its honest failed Status must
            // leave admitted files and the independent original readable.
            eprintln!("Automatic backup needs attention: {error}");
        }
        Ok(())
    }
}
