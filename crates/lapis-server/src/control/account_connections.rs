use super::{Control, engine, passwords, unavailable, validate_time};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use std::io;
use subtle::ConstantTimeEq;
use uuid::Uuid;

pub const ACCOUNT_CONNECTION_LIFETIME_MS: i64 = 10 * 60_000;
pub const ACCOUNT_CREDENTIAL_LIFETIME_MS: i64 = 30 * 86_400_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountConnectionDecision {
    Approve,
    Reject,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountConnectionStatus {
    Pending,
    Approved,
    Rejected,
    Consumed,
    Expired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountConnection {
    pub id: Uuid,
    pub client_id: Uuid,
    pub client_name: String,
    pub expires_at: i64,
    pub status: AccountConnectionStatus,
}

// Deliberately does not derive Debug because it contains a bearer credential.
pub struct IssuedAccountCredential {
    pub credential: AccountCredential,
    pub token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountCredential {
    pub id: Uuid,
    pub client_id: Uuid,
    pub expires_at: i64,
}

impl Control {
    pub async fn create_account_connection(
        &self,
        client_id: Uuid,
        client_name: &str,
        s256_challenge: &str,
        now: i64,
    ) -> io::Result<AccountConnection> {
        validate_time(now)?;
        if client_id.is_nil() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid account connection client identity",
            ));
        }
        validate_client_name(client_name)?;
        validate_challenge(s256_challenge)?;
        let id = Uuid::new_v4();
        let expires_at = now + ACCOUNT_CONNECTION_LIFETIME_MS;
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        connection
            .execute(
                "DELETE FROM sync_account_connections WHERE expires_at<=?",
                turso::params![now],
            )
            .await
            .map_err(engine)?;
        connection.execute("DELETE FROM sync_account_outcomes WHERE credential_id IS NULL AND connection_expires_at<=?", turso::params![now]).await.map_err(engine)?;
        let changed = connection
            .execute(
                "INSERT INTO sync_account_connections(id,client_id,client_name,s256_challenge,requested_at,expires_at) SELECT ?,?,?,?,?,? WHERE (SELECT COUNT(*) FROM sync_account_connections)<128",
                turso::params![id.to_string(),client_id.to_string(),client_name,s256_challenge,now,expires_at],
            )
            .await
            .map_err(engine)?;
        if changed != 1 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Account connection capacity reached",
            ));
        }
        Ok(AccountConnection {
            id,
            client_id,
            client_name: client_name.to_owned(),
            expires_at,
            status: AccountConnectionStatus::Pending,
        })
    }

    pub async fn decide_account_connection(
        &self,
        id: Uuid,
        session: Uuid,
        decision: AccountConnectionDecision,
        now: i64,
    ) -> io::Result<AccountConnectionStatus> {
        validate_time(now)?;
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        let active = connection
            .query(
                "SELECT 1 FROM sessions WHERE id=? AND revoked_at IS NULL AND expires_at>? AND last_seen_at+?>?",
                turso::params![session.to_string(),now,super::SESSION_IDLE_MS,now],
            )
            .await
            .map_err(engine)?
            .next()
            .await
            .map_err(engine)?
            .is_some();
        if !active {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Owner session is not active",
            ));
        }
        let value = match decision {
            AccountConnectionDecision::Approve => "approved",
            AccountConnectionDecision::Reject => "rejected",
        };
        let changed = connection
            .execute(
                "UPDATE sync_account_connections SET decision=?,decided_by_session=?,decided_at=? WHERE id=? AND decision IS NULL AND consumed_at IS NULL AND expires_at>? AND NOT EXISTS(SELECT 1 FROM sync_account_outcomes WHERE connection_id=sync_account_connections.id AND cancelled_at IS NOT NULL)",
                turso::params![value,session.to_string(),now,id.to_string(),now],
            )
            .await
            .map_err(engine)?;
        if changed != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Account connection is no longer pending",
            ));
        }
        Ok(match decision {
            AccountConnectionDecision::Approve => AccountConnectionStatus::Approved,
            AccountConnectionDecision::Reject => AccountConnectionStatus::Rejected,
        })
    }

    pub async fn redeem_account_connection(
        &self,
        id: Uuid,
        verifier: &str,
        now: i64,
    ) -> io::Result<IssuedAccountCredential> {
        validate_time(now)?;
        let presented = verifier_challenge(verifier)?;
        let token = passwords::token()?;
        let credential = AccountCredential {
            id: Uuid::new_v4(),
            client_id: Uuid::nil(),
            expires_at: now + ACCOUNT_CREDENTIAL_LIFETIME_MS,
        };
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        connection
            .execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(engine)?;
        let result = self
            .redeem_in_transaction(connection, id, presented, token, credential, now)
            .await;
        if result.is_err() {
            let _ = connection.execute("ROLLBACK", ()).await;
        }
        result
    }

    async fn redeem_in_transaction(
        &self,
        connection: &turso::Connection,
        id: Uuid,
        presented: [u8; 32],
        token: String,
        mut credential: AccountCredential,
        now: i64,
    ) -> io::Result<IssuedAccountCredential> {
        let mut rows = connection
            .query(
                "SELECT client_id,s256_challenge,expires_at FROM sync_account_connections WHERE id=? AND decision='approved' AND consumed_at IS NULL AND expires_at>? AND NOT EXISTS(SELECT 1 FROM sync_account_outcomes WHERE connection_id=sync_account_connections.id AND cancelled_at IS NOT NULL)",
                turso::params![id.to_string(),now],
            )
            .await
            .map_err(engine)?;
        let row = rows.next().await.map_err(engine)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Account connection cannot be redeemed",
            )
        })?;
        credential.client_id =
            Uuid::parse_str(&row.get::<String>(0).map_err(engine)?).map_err(|_| unavailable())?;
        let expected = URL_SAFE_NO_PAD
            .decode(row.get::<String>(1).map_err(engine)?)
            .map_err(|_| unavailable())?;
        if expected.len() != 32 || !bool::from(expected.as_slice().ct_eq(&presented)) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Account connection verifier is invalid",
            ));
        }
        connection
            .execute("UPDATE sync_account_connections SET consumed_at=? WHERE id=? AND consumed_at IS NULL", turso::params![now,id.to_string()])
            .await
            .map_err(engine)?;
        connection
            .execute(
                "INSERT INTO sync_account_credentials(id,client_id,token_hash,issued_at,expires_at) VALUES(?,?,?,?,?)",
                turso::params![credential.id.to_string(),credential.client_id.to_string(),passwords::digest(&token),now,credential.expires_at],
            )
            .await
            .map_err(engine)?;
        connection.execute(
            "INSERT INTO sync_account_outcomes(connection_id,client_id,s256_challenge,connection_expires_at,credential_id) VALUES(?,?,?,?,?)",
            turso::params![id.to_string(),credential.client_id.to_string(),URL_SAFE_NO_PAD.encode(presented),row.get::<i64>(2).map_err(engine)?,credential.id.to_string()],
        ).await.map_err(engine)?;
        connection.execute("COMMIT", ()).await.map_err(engine)?;
        Ok(IssuedAccountCredential { credential, token })
    }

    pub async fn resolve_account_credential(
        &self,
        token: &str,
        now: i64,
    ) -> io::Result<Option<AccountCredential>> {
        validate_time(now)?;
        if token.len() != 43 {
            return Ok(None);
        }
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        let mut rows = connection
            .query(
                "SELECT id,client_id,expires_at FROM sync_account_credentials WHERE token_hash=? AND revoked_at IS NULL AND expires_at>?",
                turso::params![passwords::digest(token),now],
            )
            .await
            .map_err(engine)?;
        let Some(row) = rows.next().await.map_err(engine)? else {
            return Ok(None);
        };
        Ok(Some(AccountCredential {
            id: Uuid::parse_str(&row.get::<String>(0).map_err(engine)?)
                .map_err(|_| unavailable())?,
            client_id: Uuid::parse_str(&row.get::<String>(1).map_err(engine)?)
                .map_err(|_| unavailable())?,
            expires_at: row.get::<i64>(2).map_err(engine)?,
        }))
    }

    pub async fn revoke_account_credential(&self, id: Uuid, now: i64) -> io::Result<()> {
        validate_time(now)?;
        let guard = self.connection.lock().await;
        guard.as_ref().ok_or_else(unavailable)?
            .execute("UPDATE sync_account_credentials SET revoked_at=? WHERE id=? AND revoked_at IS NULL", turso::params![now,id.to_string()])
            .await.map_err(engine)?;
        Ok(())
    }
}

fn validate_client_name(value: &str) -> io::Result<()> {
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid account connection client name",
        ));
    }
    Ok(())
}

fn validate_challenge(value: &str) -> io::Result<()> {
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid S256 challenge"))?;
    if value.len() != 43 || bytes.len() != 32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid S256 challenge",
        ));
    }
    Ok(())
}

pub(super) fn verifier_challenge(value: &str) -> io::Result<[u8; 32]> {
    if !(43..=128).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid S256 verifier",
        ));
    }
    Ok(Sha256::digest(value.as_bytes()).into())
}
