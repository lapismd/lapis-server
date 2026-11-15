use super::{
    AccountConnectionStatus, Control, account_connections::verifier_challenge, engine, unavailable,
    validate_time,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use std::io;
use subtle::ConstantTimeEq;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscoveryCredentialStatus {
    NotIssued,
    Active,
    Revoked,
    Expired,
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeAccountInspection {
    pub connection_id: Uuid,
    pub client_id: Uuid,
    pub expires_at: i64,
    pub status: AccountConnectionStatus,
    pub credential_status: DiscoveryCredentialStatus,
}
struct ProofState {
    public: NativeAccountInspection,
    credential_id: Option<Uuid>,
    challenge: String,
}
impl Control {
    pub async fn inspect_native_account_connection(
        &self,
        id: Uuid,
        verifier: &str,
        now: i64,
    ) -> io::Result<NativeAccountInspection> {
        validate_time(now)?;
        let presented = verifier_challenge(verifier)?;
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        Ok(proof_state(connection, id, &presented, now).await?.public)
    }
    pub async fn cancel_native_account_connection(
        &self,
        id: Uuid,
        verifier: &str,
        now: i64,
    ) -> io::Result<NativeAccountInspection> {
        validate_time(now)?;
        let presented = verifier_challenge(verifier)?;
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;
        connection
            .execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(engine)?;
        let result = cancel(connection, id, &presented, now).await;
        match result {
            Ok(public) => {
                if let Err(error) = connection.execute("COMMIT", ()).await.map_err(engine) {
                    let _ = connection.execute("ROLLBACK", ()).await;
                    return Err(error);
                }
                Ok(public)
            }
            Err(error) => {
                let _ = connection.execute("ROLLBACK", ()).await;
                Err(error)
            }
        }
    }
}
async fn cancel(
    connection: &turso::Connection,
    id: Uuid,
    presented: &[u8; 32],
    now: i64,
) -> io::Result<NativeAccountInspection> {
    let state = proof_state(connection, id, presented, now).await?;
    if state.public.credential_status == DiscoveryCredentialStatus::Unavailable {
        return Err(denied());
    }
    match state.credential_id {
        Some(credential) => {
            connection.execute("UPDATE sync_account_credentials SET revoked_at=? WHERE id=? AND revoked_at IS NULL",turso::params![now,credential.to_string()]).await.map_err(engine)?;
            connection.execute("UPDATE sync_account_outcomes SET cancelled_at=COALESCE(cancelled_at,?) WHERE connection_id=?",turso::params![now,id.to_string()]).await.map_err(engine)?;
        }
        None => {
            connection.execute(
                "INSERT INTO sync_account_outcomes(connection_id,client_id,s256_challenge,connection_expires_at,cancelled_at) VALUES(?,?,?,?,?) ON CONFLICT(connection_id) DO NOTHING",
                turso::params![id.to_string(),state.public.client_id.to_string(),state.challenge,state.public.expires_at,now],
            ).await.map_err(engine)?;
        }
    }
    Ok(proof_state(connection, id, presented, now).await?.public)
}
async fn proof_state(
    connection: &turso::Connection,
    id: Uuid,
    presented: &[u8; 32],
    now: i64,
) -> io::Result<ProofState> {
    let mut rows=connection.query(
        "SELECT c.client_id,c.s256_challenge,c.expires_at,c.decision,c.consumed_at,o.credential_id,o.cancelled_at,k.expires_at,k.revoked_at,k.client_id FROM sync_account_connections c LEFT JOIN sync_account_outcomes o ON o.connection_id=c.id LEFT JOIN sync_account_credentials k ON k.id=o.credential_id WHERE c.id=?",
        turso::params![id.to_string()],
    ).await.map_err(engine)?;
    let row = match rows.next().await.map_err(engine)? {
        Some(row) => row,
        None => {
            let mut issued=connection.query(
                "SELECT o.client_id,o.s256_challenge,o.connection_expires_at,'approved',1,o.credential_id,o.cancelled_at,k.expires_at,k.revoked_at,k.client_id FROM sync_account_outcomes o LEFT JOIN sync_account_credentials k ON k.id=o.credential_id WHERE o.connection_id=? AND o.credential_id IS NOT NULL",
                turso::params![id.to_string()],
            ).await.map_err(engine)?;
            issued.next().await.map_err(engine)?.ok_or_else(denied)?
        }
    };
    let challenge: String = row.get(1).map_err(engine)?;
    let expected = URL_SAFE_NO_PAD
        .decode(&challenge)
        .map_err(|_| unavailable())?;
    if expected.len() != 32 || !bool::from(expected.as_slice().ct_eq(presented)) {
        return Err(denied());
    }
    let client_id =
        Uuid::parse_str(&row.get::<String>(0).map_err(engine)?).map_err(|_| unavailable())?;
    let expires_at: i64 = row.get(2).map_err(engine)?;
    let decision: Option<String> = row.get(3).map_err(engine)?;
    let consumed: Option<i64> = row.get(4).map_err(engine)?;
    let credential_id = row
        .get::<Option<String>>(5)
        .map_err(engine)?
        .map(|v| Uuid::parse_str(&v).map_err(|_| unavailable()))
        .transpose()?;
    if credential_id.is_some()
        && row.get::<Option<String>>(9).map_err(engine)?.as_deref()
            != Some(client_id.to_string().as_str())
    {
        return Err(unavailable());
    }
    let cancelled: Option<i64> = row.get(6).map_err(engine)?;
    let credential_expiry: Option<i64> = row.get(7).map_err(engine)?;
    let revoked: Option<i64> = row.get(8).map_err(engine)?;
    let status = if consumed.is_some() {
        AccountConnectionStatus::Consumed
    } else if cancelled.is_some() {
        AccountConnectionStatus::Rejected
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
    let credential_status = if credential_id.is_some() {
        if revoked.is_some() {
            DiscoveryCredentialStatus::Revoked
        } else if credential_expiry.ok_or_else(unavailable)? <= now {
            DiscoveryCredentialStatus::Expired
        } else {
            DiscoveryCredentialStatus::Active
        }
    } else if consumed.is_some() {
        DiscoveryCredentialStatus::Unavailable
    } else {
        DiscoveryCredentialStatus::NotIssued
    };
    Ok(ProofState {
        public: NativeAccountInspection {
            connection_id: id,
            client_id,
            expires_at,
            status,
            credential_status,
        },
        credential_id,
        challenge,
    })
}
fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Account handoff proof or outcome is unavailable",
    )
}
