use super::{Control, SESSION_IDLE_MS, engine, unavailable};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use std::io;
use uuid::Uuid;
use webauthn_rs::prelude::{AuthenticationResult, Passkey};

const RECENT_MS: i64 = 10 * 60 * 1000;

#[derive(Serialize)]
pub(crate) struct PasskeySummary {
    id: Uuid,
    label: String,
    created_at: i64,
    last_used_at: Option<i64>,
}

pub(crate) struct StoredPasskey {
    pub summary: PasskeySummary,
    pub key: Passkey,
}

impl Control {
    pub(crate) async fn passkeys(&self, origin: &str) -> io::Result<Vec<StoredPasskey>> {
        let guard = self.connection.lock().await;
        let mut rows = guard.as_ref().ok_or_else(unavailable)?.query(
            "SELECT id,label,created_at,last_used_at,credential FROM owner_passkeys WHERE owner_id=1 AND origin=? ORDER BY created_at,id LIMIT 20",
            [origin],
        ).await.map_err(engine)?;
        let mut records = Vec::new();
        while let Some(row) = rows.next().await.map_err(engine)? {
            records.push(StoredPasskey {
                summary: PasskeySummary {
                    id: Uuid::parse_str(&row.get::<String>(0).map_err(engine)?)
                        .map_err(|_| unavailable())?,
                    label: row.get(1).map_err(engine)?,
                    created_at: row.get(2).map_err(engine)?,
                    last_used_at: row.get(3).map_err(engine)?,
                },
                key: serde_json::from_str(&row.get::<String>(4).map_err(engine)?)
                    .map_err(|_| unavailable())?,
            });
        }
        Ok(records)
    }

    pub(crate) async fn add_passkey(
        &self,
        origin: &str,
        session: Uuid,
        label: &str,
        key: &Passkey,
        now: i64,
    ) -> io::Result<bool> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        if !recent(connection, session, now).await? {
            return Ok(false);
        }
        let record = serde_json::to_string(key).map_err(|_| unavailable())?;
        let changed = connection.execute(
            "INSERT OR IGNORE INTO owner_passkeys(id,owner_id,credential_id,origin,label,credential,created_at)
             SELECT ?,1,?,?,?,?,? WHERE (SELECT count(*) FROM owner_passkeys WHERE owner_id=1)<20",
            turso::params![Uuid::new_v4().to_string(), URL_SAFE_NO_PAD.encode(key.cred_id()), origin, label, record, now],
        ).await.map_err(engine)?;
        Ok(changed == 1)
    }

    pub(crate) async fn rename_passkey(
        &self,
        origin: &str,
        id: Uuid,
        label: &str,
    ) -> io::Result<bool> {
        let guard = self.connection.lock().await;
        let changed = guard
            .as_ref()
            .ok_or_else(unavailable)?
            .execute(
                "UPDATE owner_passkeys SET label=? WHERE id=? AND origin=? AND owner_id=1",
                turso::params![label, id.to_string(), origin],
            )
            .await
            .map_err(engine)?;
        Ok(changed == 1)
    }

    pub(crate) async fn remove_passkey(
        &self,
        origin: &str,
        session: Uuid,
        id: Uuid,
        now: i64,
    ) -> io::Result<bool> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        if !recent(connection, session, now).await? {
            return Ok(false);
        }
        connection
            .execute(
                "DELETE FROM owner_passkeys WHERE id=? AND origin=? AND owner_id=1",
                turso::params![id.to_string(), origin],
            )
            .await
            .map_err(engine)?;
        Ok(true)
    }

    pub(crate) async fn use_passkey(
        &self,
        origin: &str,
        result: &AuthenticationResult,
        now: i64,
    ) -> io::Result<bool> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        let credential_id = URL_SAFE_NO_PAD.encode(result.cred_id());
        let mut rows = connection.query(
            "SELECT credential FROM owner_passkeys WHERE credential_id=? AND origin=? AND owner_id=1",
            turso::params![credential_id.as_str(), origin],
        ).await.map_err(engine)?;
        let Some(row) = rows.next().await.map_err(engine)? else {
            return Ok(false);
        };
        let mut key: Passkey = serde_json::from_str(&row.get::<String>(0).map_err(engine)?)
            .map_err(|_| unavailable())?;
        drop(rows);
        if key.update_credential(result).is_none() {
            return Ok(false);
        }
        let record = serde_json::to_string(&key).map_err(|_| unavailable())?;
        connection
            .execute(
                "UPDATE owner_passkeys SET credential=?,last_used_at=? WHERE credential_id=?",
                turso::params![record, now, credential_id],
            )
            .await
            .map_err(engine)?;
        Ok(true)
    }

    pub(crate) async fn recently_verified(&self, session: Uuid, now: i64) -> io::Result<bool> {
        let guard = self.connection.lock().await;
        recent(guard.as_ref().ok_or_else(unavailable)?, session, now).await
    }

    pub(crate) async fn mark_verified(&self, session: Uuid, now: i64) -> io::Result<bool> {
        let guard = self.connection.lock().await;
        let changed = guard.as_ref().ok_or_else(unavailable)?.execute(
            "INSERT INTO session_reauthentication(session_id,verified_at)
             SELECT id,? FROM sessions WHERE id=? AND revoked_at IS NULL AND expires_at>? AND last_seen_at+?>?
             ON CONFLICT(session_id) DO UPDATE SET verified_at=excluded.verified_at",
            turso::params![now, session.to_string(), now, SESSION_IDLE_MS, now],
        ).await.map_err(engine)?;
        Ok(changed == 1)
    }

    pub(crate) async fn owner_name(&self) -> io::Result<Option<String>> {
        Ok(self.owner().await?.map(|owner| owner.username))
    }

    pub(crate) async fn passkey_login_allowed(&self, peer: &str, now: i64) -> io::Result<bool> {
        self.login_allowed(&Self::login_key("passkey", peer), now)
            .await
    }

    pub(crate) async fn passkey_login_failed(&self, peer: &str, now: i64) -> io::Result<()> {
        self.record_failure(&Self::login_key("passkey", peer), now)
            .await
    }
}

async fn recent(connection: &turso::Connection, session: Uuid, now: i64) -> io::Result<bool> {
    let mut rows = connection
        .query(
            "SELECT 1 FROM sessions s LEFT JOIN session_reauthentication r ON s.id=r.session_id
         WHERE s.id=? AND s.revoked_at IS NULL AND s.expires_at>? AND s.last_seen_at+?>?
           AND MAX(s.created_at,COALESCE(r.verified_at,0))>?",
            turso::params![
                session.to_string(),
                now,
                SESSION_IDLE_MS,
                now,
                now.saturating_sub(RECENT_MS)
            ],
        )
        .await
        .map_err(engine)?;
    Ok(rows.next().await.map_err(engine)?.is_some())
}
