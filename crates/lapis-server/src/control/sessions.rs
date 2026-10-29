use super::{
    Control, SESSION_ABSOLUTE_MS, SESSION_IDLE_MS, engine, passwords, unavailable, validate_time,
};
use std::io;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Session {
    pub id: Uuid,
    pub username: String,
    pub csrf_token: String,
    pub expires_at: i64,
}
pub struct IssuedSession {
    pub session: Session,
    pub token: String,
}
impl Control {
    pub async fn issue(&self, now: i64) -> io::Result<IssuedSession> {
        validate_time(now)?;
        let token = passwords::token()?;
        let csrf_token = passwords::token()?;
        let id = Uuid::new_v4();
        let expires_at = now + SESSION_ABSOLUTE_MS;
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        let username = {
            let mut rows = connection
                .query("SELECT username FROM installation_owner WHERE id=1", ())
                .await
                .map_err(engine)?;
            rows.next()
                .await
                .map_err(engine)?
                .ok_or_else(unavailable)?
                .get::<String>(0)
                .map_err(engine)?
        };
        connection.execute("DELETE FROM sessions WHERE revoked_at IS NOT NULL OR expires_at<=? OR last_seen_at+?<=?", turso::params![now,SESSION_IDLE_MS,now]).await.map_err(engine)?;
        connection.execute("INSERT INTO sessions(id,owner_id,token_hash,csrf_token,created_at,last_seen_at,expires_at) VALUES(?,1,?,?,?,?,?)", turso::params![id.to_string(),passwords::digest(&token),csrf_token.as_str(),now,now,expires_at]).await.map_err(engine)?;
        Ok(IssuedSession {
            session: Session {
                id,
                username,
                csrf_token,
                expires_at,
            },
            token,
        })
    }
    pub async fn resolve(&self, token: &str, now: i64) -> io::Result<Option<Session>> {
        validate_time(now)?;
        if token.len() != 43 {
            return Ok(None);
        }
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        let record = {
            let mut rows = connection.query("SELECT s.id,o.username,s.csrf_token,s.expires_at,s.last_seen_at,s.revoked_at FROM sessions s JOIN installation_owner o ON o.id=s.owner_id WHERE s.token_hash=?", turso::params![passwords::digest(token)]).await.map_err(engine)?;
            let Some(row) = rows.next().await.map_err(engine)? else {
                return Ok(None);
            };
            let session = Session {
                id: Uuid::parse_str(&row.get::<String>(0).map_err(engine)?)
                    .map_err(|_| unavailable())?,
                username: row.get::<String>(1).map_err(engine)?,
                csrf_token: row.get::<String>(2).map_err(engine)?,
                expires_at: row.get::<i64>(3).map_err(engine)?,
            };
            (
                session,
                row.get::<i64>(4).map_err(engine)?,
                !matches!(row.get_value(5).map_err(engine)?, turso::Value::Null),
            )
        };
        let (session, last_seen, revoked) = record;
        if revoked
            || session.expires_at <= now
            || last_seen
                .checked_add(SESSION_IDLE_MS)
                .is_none_or(|deadline| deadline <= now)
        {
            connection
                .execute(
                    "UPDATE sessions SET revoked_at=? WHERE id=? AND revoked_at IS NULL",
                    turso::params![now, session.id.to_string()],
                )
                .await
                .map_err(engine)?;
            return Ok(None);
        }
        connection
            .execute(
                "UPDATE sessions SET last_seen_at=MAX(last_seen_at,?) WHERE id=?",
                turso::params![now, session.id.to_string()],
            )
            .await
            .map_err(engine)?;
        Ok(Some(session))
    }
    pub async fn revoke(&self, id: Uuid, now: i64) -> io::Result<()> {
        validate_time(now)?;
        let guard = self.connection.lock().await;
        guard
            .as_ref()
            .ok_or_else(unavailable)?
            .execute(
                "UPDATE sessions SET revoked_at=? WHERE id=? AND revoked_at IS NULL",
                turso::params![now, id.to_string()],
            )
            .await
            .map_err(engine)?;
        Ok(())
    }
}
