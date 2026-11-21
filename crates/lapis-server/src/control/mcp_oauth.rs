mod refresh;
use refresh::refresh;

use super::{Control, engine, passwords, unavailable};
use lapis_app_protocol::content::McpConnectionSummary;
use serde::{Serialize, de::DeserializeOwned};
use std::{collections::BTreeMap, io};
use uuid::Uuid;

pub(crate) const ACCESS_SECONDS: i64 = 900;
const MAX_AGE_MS: i64 = 90 * 86_400_000;

#[derive(Clone)]
pub(crate) struct Client {
    pub id: String,
    pub name: String,
    pub redirects: Vec<String>,
    pub auth_method: String,
    pub secret_verifier: Option<String>,
}

pub(crate) struct Consent {
    pub client_id: String,
    pub resource: String,
    pub label: String,
    pub vault_ids: Vec<Uuid>,
    pub permissions: Vec<String>,
    pub operations: BTreeMap<String, String>,
    pub scope: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub(crate) struct TokenPair {
    access_token: String,
    #[serde(rename = "token_type")]
    kind: &'static str,
    expires_in: i64,
    refresh_token: String,
    scope: String,
}

impl TokenPair {
    fn new(scope: String) -> io::Result<Self> {
        Ok(Self {
            access_token: format!("lo1_{}", passwords::token()?),
            kind: "Bearer",
            expires_in: ACCESS_SECONDS,
            refresh_token: format!("lor_{}", passwords::token()?),
            scope,
        })
    }
}

pub(crate) enum Refresh {
    Issued(Uuid, TokenPair),
    Replayed(Uuid),
    Invalid,
}

const SUMMARY: &str =
    "id,label,vault_ids,permissions,created_at,expires_at,last_used_at,revoked_at";

impl Control {
    pub(crate) async fn register_mcp_client(&self, client: &Client, now: i64) -> io::Result<bool> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        connection.execute(
            "DELETE FROM mcp_oauth_clients WHERE created_at<? AND NOT EXISTS (SELECT 1 FROM mcp_oauth_grants WHERE client_id=mcp_oauth_clients.id AND revoked_at IS NULL AND expires_at>?)",
            turso::params![now - 86_400_000, now],
        ).await.map_err(engine)?;
        let count = connection.execute(
            "INSERT INTO mcp_oauth_clients(id,name,redirects,auth_method,secret_verifier,created_at)
             SELECT ?,?,?,?,?,? WHERE (SELECT count(*) FROM mcp_oauth_clients)<256",
            turso::params![client.id.as_str(), client.name.as_str(), encode(&client.redirects)?,
                client.auth_method.as_str(), client.secret_verifier.as_deref(), now],
        ).await.map_err(engine)?;
        Ok(count == 1)
    }

    pub(crate) async fn mcp_client(&self, id: &str) -> io::Result<Option<Client>> {
        let guard = self.connection.lock().await;
        let mut rows = guard.as_ref().ok_or_else(unavailable)?.query(
            "SELECT name,redirects,auth_method,secret_verifier FROM mcp_oauth_clients WHERE id=?",
            [id],
        ).await.map_err(engine)?;
        rows.next()
            .await
            .map_err(engine)?
            .map(|row| {
                Ok(Client {
                    id: id.into(),
                    name: row.get(0).map_err(engine)?,
                    redirects: decode(row.get::<String>(1).map_err(engine)?)?,
                    auth_method: row.get(2).map_err(engine)?,
                    secret_verifier: row.get(3).map_err(engine)?,
                })
            })
            .transpose()
    }

    pub(crate) async fn issue_mcp(
        &self,
        consent: &Consent,
        now: i64,
    ) -> io::Result<Option<TokenPair>> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        let token = TokenPair::new(consent.scope.clone())?;
        connection
            .execute(
                "DELETE FROM mcp_oauth_grants WHERE expires_at<=? OR revoked_at<=?",
                turso::params![now, now - 30 * 86_400_000],
            )
            .await
            .map_err(engine)?;
        let count = connection.execute(
            "INSERT INTO mcp_oauth_grants(id,owner_id,client_id,resource,label,vault_ids,permissions,operations,scope,created_at,
              expires_at,access_verifier,access_expires_at,refresh_verifier)
             SELECT ?,1,?,?,?,?,?,?,?,?,?,?,?,? WHERE (SELECT count(*) FROM mcp_oauth_grants)<256",
            turso::params![Uuid::new_v4().to_string(), consent.client_id.as_str(), consent.resource.as_str(), consent.label.as_str(),
                encode(&consent.vault_ids)?, encode(&consent.permissions)?, encode(&consent.operations)?, consent.scope.as_str(),
                now, now + MAX_AGE_MS, passwords::digest(&token.access_token), now + ACCESS_SECONDS * 1000,
                passwords::digest(&token.refresh_token)],
        ).await.map_err(engine)?;
        Ok((count == 1).then_some(token))
    }

    pub(crate) async fn refresh_mcp(
        &self,
        client: &str,
        resource: &str,
        bearer: &str,
        now: i64,
    ) -> io::Result<Refresh> {
        let guard = self.connection.lock().await;
        let db = guard.as_ref().ok_or_else(unavailable)?;
        db.execute("BEGIN IMMEDIATE", ()).await.map_err(engine)?;
        let result = refresh(db, client, resource, &passwords::digest(bearer), now).await;
        match result {
            Ok(value) => match db.execute("COMMIT", ()).await {
                Ok(_) => Ok(value),
                Err(error) => {
                    let _ = db.execute("ROLLBACK", ()).await;
                    Err(engine(error))
                }
            },
            Err(error) => {
                let _ = db.execute("ROLLBACK", ()).await;
                Err(error)
            }
        }
    }

    pub(crate) async fn mcp_approval(
        &self,
        bearer: &str,
        resource: &str,
        now: i64,
    ) -> io::Result<Option<super::personal_tokens::Approval>> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        let mut rows = connection.query(
            &format!("SELECT {SUMMARY},operations,access_expires_at FROM mcp_oauth_grants
             WHERE access_verifier=? AND resource=? AND revoked_at IS NULL AND expires_at>? AND access_expires_at>?"),
            turso::params![passwords::digest(bearer), resource, now, now],
        ).await.map_err(engine)?;
        rows.next()
            .await
            .map_err(engine)?
            .map(|row| {
                let mut grant = super::personal_tokens::summary(&row, now)?;
                let access_expires_at: i64 = row.get(9).map_err(engine)?;
                grant.expires_at = grant
                    .expires_at
                    .map(|expires| expires.min(access_expires_at));
                Ok(super::personal_tokens::Approval {
                    summary: grant,
                    operations: decode(row.get::<String>(8).map_err(engine)?)?,
                })
            })
            .transpose()
    }

    pub(crate) async fn connected_mcp(&self, now: i64) -> io::Result<Vec<McpConnectionSummary>> {
        let guard = self.connection.lock().await;
        let mut rows = guard.as_ref().ok_or_else(unavailable)?.query(
            &format!("SELECT {SUMMARY},client_id FROM mcp_oauth_grants ORDER BY revoked_at IS NOT NULL,created_at DESC LIMIT 256"), (),
        ).await.map_err(engine)?;
        let mut clients = Vec::new();
        while let Some(row) = rows.next().await.map_err(engine)? {
            clients.push(McpConnectionSummary {
                client_id: row.get(8).map_err(engine)?,
                grant: super::personal_tokens::summary(&row, now)?,
            });
        }
        Ok(clients)
    }

    pub(crate) async fn disconnect_mcp(&self, id: Uuid, now: i64) -> io::Result<()> {
        let guard = self.connection.lock().await;
        revoke(guard.as_ref().ok_or_else(unavailable)?, id, now).await
    }

    pub(crate) async fn revoke_mcp_token(
        &self,
        client: &str,
        resource: &str,
        token: &str,
        now: i64,
    ) -> io::Result<Option<Uuid>> {
        let guard = self.connection.lock().await;
        let db = guard.as_ref().ok_or_else(unavailable)?;
        let digest = passwords::digest(token);
        let mut rows = db.query(
            "SELECT id FROM mcp_oauth_grants WHERE client_id=? AND resource=? AND (access_verifier=? OR refresh_verifier=?) AND revoked_at IS NULL",
            turso::params![client, resource, digest.as_str(), digest.as_str()],
        ).await.map_err(engine)?;
        let id = rows
            .next()
            .await
            .map_err(engine)?
            .map(|row| uuid(&row))
            .transpose()?;
        drop(rows);
        if let Some(id) = id {
            revoke(db, id, now).await?;
        }
        Ok(id)
    }

    pub(crate) async fn touch_mcp(&self, id: Uuid, now: i64) -> io::Result<()> {
        let guard = self.connection.lock().await;
        guard.as_ref().ok_or_else(unavailable)?.execute(
            "UPDATE mcp_oauth_grants SET last_used_at=? WHERE id=? AND revoked_at IS NULL AND (last_used_at IS NULL OR last_used_at<?)",
            turso::params![now, id.to_string(), now.saturating_sub(60_000)],
        ).await.map_err(engine)?;
        Ok(())
    }
}

async fn revoke(db: &turso::Connection, id: Uuid, now: i64) -> io::Result<()> {
    db.execute(
        "UPDATE mcp_oauth_grants SET revoked_at=? WHERE id=? AND revoked_at IS NULL",
        turso::params![now, id.to_string()],
    )
    .await
    .map_err(engine)?;
    Ok(())
}

fn uuid(row: &turso::Row) -> io::Result<Uuid> {
    Uuid::parse_str(&row.get::<String>(0).map_err(engine)?).map_err(|_| unavailable())
}
fn encode(value: &impl Serialize) -> io::Result<String> {
    serde_json::to_string(value).map_err(|_| unavailable())
}
fn decode<T: DeserializeOwned>(value: String) -> io::Result<T> {
    serde_json::from_str(&value).map_err(|_| unavailable())
}
