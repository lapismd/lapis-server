use super::{
    AccountConnection, AccountConnectionStatus, Control, engine, unavailable, validate_time,
};
use std::io;
use uuid::Uuid;

impl Control {
    pub(crate) async fn discover_account_vaults(
        &self,
    ) -> io::Result<Vec<lapis_app_protocol::ManagedVault>> {
        let guard = self.connection.lock().await;
        let mut rows = guard.as_ref().ok_or_else(unavailable)?.query(
            "SELECT id,name FROM hosted_vaults WHERE managed=1 AND archived_at IS NULL ORDER BY id LIMIT 4097", (),
        ).await.map_err(engine)?;
        let mut vaults = Vec::new();
        while let Some(row) = rows.next().await.map_err(engine)? {
            if vaults.len() == 4096 {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Vault discovery capacity reached",
                ));
            }
            vaults.push(lapis_app_protocol::ManagedVault {
                vault_id: Uuid::parse_str(&row.get::<String>(0).map_err(engine)?)
                    .map_err(|_| unavailable())?,
                name: row.get(1).map_err(engine)?,
                archived_at: None,
            });
        }
        Ok(vaults)
    }
    /// Read only display metadata; neither verifier material nor bearer hashes
    /// belong in the browser consent response.
    pub async fn inspect_account_connection(
        &self,
        id: Uuid,
        now: i64,
    ) -> io::Result<Option<AccountConnection>> {
        validate_time(now)?;
        let guard = self.connection.lock().await;
        let mut rows = guard.as_ref().ok_or_else(unavailable)?.query(
            "SELECT client_id,client_name,expires_at,decision,consumed_at FROM sync_account_connections WHERE id=?",
            turso::params![id.to_string()],
        ).await.map_err(engine)?;
        let Some(row) = rows.next().await.map_err(engine)? else {
            return Ok(None);
        };
        let expires_at: i64 = row.get(2).map_err(engine)?;
        let decision: Option<String> = row.get(3).map_err(engine)?;
        let consumed: Option<i64> = row.get(4).map_err(engine)?;
        let status = if consumed.is_some() {
            AccountConnectionStatus::Consumed
        } else if expires_at <= now {
            AccountConnectionStatus::Expired
        } else {
            match decision.as_deref() {
                None => AccountConnectionStatus::Pending,
                Some("approved") => AccountConnectionStatus::Approved,
                Some("rejected") => AccountConnectionStatus::Rejected,
                _ => return Err(unavailable()),
            }
        };
        Ok(Some(AccountConnection {
            id,
            client_id: Uuid::parse_str(&row.get::<String>(0).map_err(engine)?)
                .map_err(|_| unavailable())?,
            client_name: row.get(1).map_err(engine)?,
            expires_at,
            status,
        }))
    }
}
