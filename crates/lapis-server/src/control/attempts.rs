use super::{Control, engine, passwords, unavailable, validate_time};
use std::io;

impl Control {
    pub fn login_key(username: &str, peer: &str) -> String {
        passwords::digest(&format!("{}\0{peer}", username.to_lowercase()))
    }
    pub(super) async fn login_allowed(&self, key: &str, now: i64) -> io::Result<bool> {
        validate_time(now)?;
        let guard = self.connection.lock().await;
        let mut rows = guard
            .as_ref()
            .ok_or_else(unavailable)?
            .query(
                "SELECT COALESCE(blocked_until,0) FROM login_attempts WHERE key_hash=?",
                [key],
            )
            .await
            .map_err(engine)?;
        Ok(rows
            .next()
            .await
            .map_err(engine)?
            .map(|row| row.get::<i64>(0))
            .transpose()
            .map_err(engine)?
            .is_none_or(|until| until <= now))
    }
    pub async fn record_failure(&self, key: &str, now: i64) -> io::Result<()> {
        validate_time(now)?;
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        let previous = {
            let mut rows = connection
                .query(
                    "SELECT window_started_at,attempt_count FROM login_attempts WHERE key_hash=?",
                    [key],
                )
                .await
                .map_err(engine)?;
            rows.next()
                .await
                .map_err(engine)?
                .map(|row| {
                    Ok::<_, io::Error>((
                        row.get::<i64>(0).map_err(engine)?,
                        row.get::<i64>(1).map_err(engine)?,
                    ))
                })
                .transpose()?
        };
        let (start, count) = previous
            .filter(|(start, _)| start.saturating_add(10 * 60_000) > now)
            .map_or((now, 1), |(start, count)| (start, count.saturating_add(1)));
        let blocked = if count >= 8 {
            turso::Value::Integer(now + 15 * 60_000)
        } else {
            turso::Value::Null
        };
        connection.execute("DELETE FROM login_attempts WHERE window_started_at+600000<=? AND COALESCE(blocked_until,0)<=?", turso::params![now,now]).await.map_err(engine)?;
        connection.execute("INSERT INTO login_attempts(key_hash,window_started_at,attempt_count,blocked_until) VALUES(?,?,?,?) ON CONFLICT(key_hash) DO UPDATE SET window_started_at=excluded.window_started_at,attempt_count=excluded.attempt_count,blocked_until=excluded.blocked_until", turso::params![key,start,count,blocked]).await.map_err(engine)?;
        Ok(())
    }
    pub(super) async fn clear_failures(&self, key: &str) -> io::Result<()> {
        let guard = self.connection.lock().await;
        guard
            .as_ref()
            .ok_or_else(unavailable)?
            .execute("DELETE FROM login_attempts WHERE key_hash=?", [key])
            .await
            .map_err(engine)?;
        Ok(())
    }
}
