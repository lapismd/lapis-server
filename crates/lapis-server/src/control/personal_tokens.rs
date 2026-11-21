use super::{Control, engine, passwords, unavailable};
use lapis_app_protocol::content::{PersonalTokenRequest, PersonalTokenSummary};
use serde::de::DeserializeOwned;
use std::{collections::BTreeMap, io};
use uuid::Uuid;

pub(crate) struct Approval {
    pub summary: PersonalTokenSummary,
    pub operations: BTreeMap<String, String>,
}

const SUMMARY: &str =
    "id,label,vault_ids,permissions,created_at,expires_at,last_used_at,revoked_at";
const LIMIT: i64 = 128;

impl Control {
    pub(crate) async fn create_personal_token(
        &self,
        request: PersonalTokenRequest,
        operations: BTreeMap<String, String>,
        now: i64,
    ) -> io::Result<(Uuid, String)> {
        let id = Uuid::new_v4();
        let secret = format!("lp1_{}", passwords::token()?);
        let expires = request
            .lifetime
            .seconds()
            .map(|seconds| now + seconds * 1000);
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        // Keep bounded history without limiting replacement of expired or revoked credentials.
        connection.execute(
            "DELETE FROM personal_access_tokens WHERE id IN (SELECT id FROM personal_access_tokens WHERE revoked_at IS NOT NULL OR expires_at<=? ORDER BY created_at DESC LIMIT -1 OFFSET 127)",
            [now],
        ).await.map_err(engine)?;
        let inserted = connection.execute(
            "INSERT INTO personal_access_tokens(id,owner_id,verifier,label,vault_ids,permissions,operations,created_at,expires_at)
             SELECT ?,1,?,?,?,?,?,?,? WHERE (SELECT count(*) FROM personal_access_tokens WHERE revoked_at IS NULL AND (expires_at IS NULL OR expires_at>?))<?",
            turso::params![id.to_string(), passwords::digest(&secret), request.label.as_str(),
                encode(&request.vault_ids)?, encode(&request.permissions)?, encode(&operations)?, now, expires, now, LIMIT],
        ).await.map_err(engine)?;
        if inserted != 1 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Personal token limit reached",
            ));
        }
        Ok((id, secret))
    }

    pub(crate) async fn personal_tokens(&self, now: i64) -> io::Result<Vec<PersonalTokenSummary>> {
        let guard = self.connection.lock().await;
        let mut rows = guard.as_ref().ok_or_else(unavailable)?.query(
            &format!("SELECT {SUMMARY} FROM personal_access_tokens ORDER BY (revoked_at IS NULL AND (expires_at IS NULL OR expires_at>?)) DESC, created_at DESC, id LIMIT 256"),
            [now],
        ).await.map_err(engine)?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await.map_err(engine)? {
            result.push(summary(&row, now)?);
        }
        Ok(result)
    }

    pub(crate) async fn personal_approval(
        &self,
        bearer: &str,
        now: i64,
    ) -> io::Result<Option<Approval>> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        let digest = passwords::digest(bearer);
        let mut rows = connection.query(
            &format!("SELECT {SUMMARY},operations FROM personal_access_tokens WHERE verifier=? AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>?)"),
            turso::params![digest, now],
        ).await.map_err(engine)?;
        let approval = rows
            .next()
            .await
            .map_err(engine)?
            .map(|row| {
                Ok::<_, io::Error>(Approval {
                    summary: summary(&row, now)?,
                    operations: decode(row.get::<String>(8).map_err(engine)?)?,
                })
            })
            .transpose()?;
        Ok(approval)
    }

    pub(crate) async fn touch_personal(&self, id: Uuid, now: i64) -> io::Result<()> {
        let guard = self.connection.lock().await;
        guard.as_ref().ok_or_else(unavailable)?.execute(
            "UPDATE personal_access_tokens SET last_used_at=? WHERE id=? AND revoked_at IS NULL AND (last_used_at IS NULL OR last_used_at<?)",
            turso::params![now, id.to_string(), now.saturating_sub(60_000)],
        ).await.map_err(engine)?;
        Ok(())
    }

    pub(crate) async fn revoke_personal(&self, id: Uuid, now: i64) -> io::Result<()> {
        let guard = self.connection.lock().await;
        guard
            .as_ref()
            .ok_or_else(unavailable)?
            .execute(
                "UPDATE personal_access_tokens SET revoked_at=? WHERE id=? AND revoked_at IS NULL",
                turso::params![now, id.to_string()],
            )
            .await
            .map_err(engine)?;
        Ok(())
    }

    pub(crate) async fn revoke_personal_for_vault(&self, id: Uuid, now: i64) -> io::Result<()> {
        let guard = self.connection.lock().await;
        let db = guard.as_ref().ok_or_else(unavailable)?;
        db.execute("BEGIN IMMEDIATE", ()).await.map_err(engine)?;
        // These lists contain only canonical UUID strings emitted by serde.
        let encoded_id = format!("\"{id}\"");
        let result: io::Result<()> = async {
            db.execute(
                "UPDATE personal_access_tokens SET revoked_at=? WHERE revoked_at IS NULL AND instr(vault_ids,?)>0",
                turso::params![now, encoded_id.as_str()],
            ).await.map_err(engine)?;
            db.execute(
                "UPDATE mcp_oauth_grants SET revoked_at=? WHERE revoked_at IS NULL AND instr(vault_ids,?)>0",
                turso::params![now, encoded_id.as_str()],
            ).await.map_err(engine)?;
            db.execute("COMMIT", ()).await.map_err(engine)?;
            Ok(())
        }.await;
        if result.is_err() {
            let _ = db.execute("ROLLBACK", ()).await;
        }
        result?;
        Ok(())
    }
}

pub(super) fn summary(row: &turso::Row, now: i64) -> io::Result<PersonalTokenSummary> {
    let expires_at: Option<i64> = row.get(5).map_err(engine)?;
    let revoked_at: Option<i64> = row.get(7).map_err(engine)?;
    Ok(PersonalTokenSummary {
        grant_id: Uuid::parse_str(&row.get::<String>(0).map_err(engine)?)
            .map_err(|_| unavailable())?,
        label: row.get(1).map_err(engine)?,
        vault_ids: decode(row.get::<String>(2).map_err(engine)?)?,
        permissions: decode(row.get::<String>(3).map_err(engine)?)?,
        created_at: row.get(4).map_err(engine)?,
        last_used_at: row.get(6).map_err(engine)?,
        active: revoked_at.is_none() && expires_at.is_none_or(|deadline| deadline > now),
        expires_at,
    })
}

fn encode(value: &impl serde::Serialize) -> io::Result<String> {
    serde_json::to_string(value).map_err(|_| unavailable())
}

fn decode<T: DeserializeOwned>(value: String) -> io::Result<T> {
    serde_json::from_str(&value).map_err(|_| unavailable())
}
