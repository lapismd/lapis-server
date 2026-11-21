use super::{Control, engine, unavailable};
use std::io;

impl Control {
    pub(crate) async fn content_enabled(&self) -> io::Result<bool> {
        let guard = self.connection.lock().await;
        let mut rows = guard
            .as_ref()
            .ok_or_else(unavailable)?
            .query("SELECT enabled FROM content_access_settings WHERE id=1", ())
            .await
            .map_err(engine)?;
        match rows.next().await.map_err(engine)? {
            None => Ok(false),
            Some(row) => match row.get::<i64>(0).map_err(engine)? {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(unavailable()),
            },
        }
    }

    pub(crate) async fn set_content_enabled(&self, enabled: bool) -> io::Result<()> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        if !enabled {
            connection
                .execute("BEGIN IMMEDIATE", ())
                .await
                .map_err(engine)?;
        }
        let result = async {
            if !enabled {
                connection.execute(
                    "UPDATE personal_access_tokens SET revoked_at=? WHERE revoked_at IS NULL",
                    [super::now_ms()?],
                ).await.map_err(engine)?;
                connection.execute(
                    "UPDATE mcp_oauth_grants SET revoked_at=? WHERE revoked_at IS NULL",
                    [super::now_ms()?],
                ).await.map_err(engine)?;
            }
            connection.execute(
            "INSERT INTO content_access_settings(id,enabled) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET enabled=excluded.enabled",
            [i64::from(enabled)],
            ).await.map_err(engine)?;
            if !enabled {
                connection.execute("COMMIT", ()).await.map_err(engine)?;
            }
            Ok(())
        }.await;
        if !enabled && result.is_err() {
            let _ = connection.execute("ROLLBACK", ()).await;
        }
        result
    }
}
