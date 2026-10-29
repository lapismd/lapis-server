use super::{Control, engine, passwords, unavailable, validate_time};
use std::io;

pub(super) struct Owner {
    pub username: String,
    pub password_hash: String,
}
impl Control {
    pub async fn initialize(&self, username: &str, encoded: &str, now: i64) -> io::Result<bool> {
        validate_time(now)?;
        if username.trim().is_empty()
            || username.encode_utf16().count() > 200
            || !passwords::valid(encoded)
        {
            return Err(io::Error::other("Invalid installation owner credentials"));
        }
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        Ok(connection.execute("INSERT OR IGNORE INTO installation_owner(id,username,password_hash,created_at) VALUES(1,?,?,?)", turso::params![username,encoded,now]).await.map_err(engine)? > 0)
    }
    pub async fn initialized(&self) -> io::Result<bool> {
        Ok(self.owner().await?.is_some())
    }
    pub(super) async fn owner(&self) -> io::Result<Option<Owner>> {
        let guard = self.connection.lock().await;
        let mut rows = guard
            .as_ref()
            .ok_or_else(unavailable)?
            .query(
                "SELECT username,password_hash FROM installation_owner WHERE id=1",
                (),
            )
            .await
            .map_err(engine)?;
        rows.next()
            .await
            .map_err(engine)?
            .map(|row| {
                Ok(Owner {
                    username: row.get::<String>(0).map_err(engine)?,
                    password_hash: row.get::<String>(1).map_err(engine)?,
                })
            })
            .transpose()
    }
}
