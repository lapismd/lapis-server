use super::{Control, engine, unavailable};
use lapis_app_protocol::ManagedVault;
use std::io;
use uuid::Uuid;

pub(crate) struct StoredVault {
    pub vault: ManagedVault,
    pub managed: bool,
    pub identity: (u64, u64),
}

impl Control {
    pub(crate) async fn vaults(&self) -> io::Result<Vec<StoredVault>> {
        let guard = self.connection.lock().await;
        let mut rows = guard.as_ref().ok_or_else(unavailable)?
            .query("SELECT id,name,managed,archived_at,physical_device,physical_inode FROM hosted_vaults ORDER BY rowid", ())
            .await.map_err(engine)?;
        let mut vaults = Vec::new();
        while let Some(row) = rows.next().await.map_err(engine)? {
            vaults.push(StoredVault {
                vault: ManagedVault {
                    vault_id: Uuid::parse_str(&row.get::<String>(0).map_err(engine)?)
                        .map_err(|_| unavailable())?,
                    name: row.get::<String>(1).map_err(engine)?,
                    archived_at: match row.get_value(3).map_err(engine)? {
                        turso::Value::Null => None,
                        turso::Value::Integer(value) => Some(value),
                        _ => return Err(unavailable()),
                    },
                },
                managed: row.get::<i64>(2).map_err(engine)? == 1,
                identity: (
                    row.get::<String>(4)
                        .map_err(engine)?
                        .parse()
                        .map_err(|_| unavailable())?,
                    row.get::<String>(5)
                        .map_err(engine)?
                        .parse()
                        .map_err(|_| unavailable())?,
                ),
            });
        }
        Ok(vaults)
    }

    pub(crate) async fn insert_vault(
        &self,
        vault: &ManagedVault,
        managed: bool,
        identity: (u64, u64),
    ) -> io::Result<()> {
        let guard = self.connection.lock().await;
        guard.as_ref().ok_or_else(unavailable)?.execute(
            "INSERT INTO hosted_vaults(id,owner_id,name,managed,archived_at,physical_device,physical_inode) VALUES(?,1,?,?,NULL,?,?)",
            turso::params![vault.vault_id.to_string(), vault.name.as_str(), i64::from(managed), identity.0.to_string(), identity.1.to_string()],
        ).await.map_err(engine)?;
        Ok(())
    }

    pub(crate) async fn update_vault(&self, vault: &ManagedVault) -> io::Result<()> {
        let guard = self.connection.lock().await;
        let archived = vault
            .archived_at
            .map_or(turso::Value::Null, turso::Value::Integer);
        let count = guard
            .as_ref()
            .ok_or_else(unavailable)?
            .execute(
                "UPDATE hosted_vaults SET name=?,archived_at=? WHERE id=? AND owner_id=1",
                turso::params![vault.name.as_str(), archived, vault.vault_id.to_string()],
            )
            .await
            .map_err(engine)?;
        if count != 1 {
            return Err(unavailable());
        }
        Ok(())
    }
}
