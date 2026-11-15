//! Host-owned bounded account transport. No endpoint enrollment or content authority.
pub mod owner;
pub mod protected;
mod recovery;
pub mod session;
mod types;
pub use recovery::{DiscoveryCredentialStatus, HandoffInspection, HandoffStatus};
use reqwest::{
    Method, Url,
    blocking::{Client, RequestBuilder},
    redirect::Policy,
};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::{
    collections::HashSet,
    io::{self, Read},
    time::Duration,
};
pub use types::{AccountConnection, AccountVerifier, DiscoveredVault, DiscoveryCredential};
use types::{ConnectionWire, CredentialWire, SignOutWire, VaultsWire, valid_expiry, valid_name};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone, Copy)]
pub enum AccountTransportPolicy {
    Https,
    LoopbackDevelopment,
}

/// Own this in a native service, off the async executor and renderer thread.
/// Mutating requests are sent once. The service must persist intent and inspect
/// uncertain outcomes; this transport never retries or falls back.
pub struct AccountEndpoint {
    origin: String,
    client: Client,
}
impl AccountEndpoint {
    pub fn open(origin: &str, policy: AccountTransportPolicy) -> io::Result<Self> {
        if origin.len() > 2048 || origin.chars().any(char::is_control) {
            return Err(invalid());
        }
        let url = Url::parse(origin).map_err(|_| invalid())?;
        let local = matches!(policy, AccountTransportPolicy::LoopbackDevelopment)
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if (url.scheme() != "https" && !local)
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(invalid());
        }
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| unavailable())?;
        Ok(Self {
            origin: url.origin().ascii_serialization(),
            client,
        })
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn initiate(
        &self,
        client_id: Uuid,
        client_name: &str,
        verifier: &AccountVerifier,
        now: i64,
    ) -> io::Result<AccountConnection> {
        if client_id.is_nil() || !valid_name(client_name, 128) || now < 0 {
            return Err(invalid());
        }
        let body = json!({"client_id":client_id,"client_name":client_name,"s256_challenge":verifier.challenge()});
        let wire: ConnectionWire = self.json(
            self.request(Method::POST, "/api/v3/sync/account")
                .body(body.to_string()),
            4096,
        )?;
        let expected = format!("{}/?sync-connect={}", self.origin, wire.id);
        if wire.id.is_nil()
            || wire.client_id != client_id
            || wire.client_name != client_name
            || !valid_expiry(wire.expires_at, now, 10 * 60_000)
            || wire.approval_url != expected
        {
            return Err(invalid());
        }
        Ok(AccountConnection {
            id: wire.id,
            client_id: wire.client_id,
            client_name: wire.client_name,
            expires_at: wire.expires_at,
            approval_url: wire.approval_url,
        })
    }
    pub fn redeem(
        &self,
        id: Uuid,
        client_id: Uuid,
        verifier: &AccountVerifier,
        now: i64,
    ) -> io::Result<DiscoveryCredential> {
        if id.is_nil() || client_id.is_nil() || now < 0 {
            return Err(invalid());
        }
        // Request storage is owned by reqwest until the single send completes.
        let body = format!("{{\"verifier\":\"{}\"}}", verifier.expose_secret());
        let wire: CredentialWire = self.json(
            self.request(Method::POST, &format!("/api/v3/sync/account/{id}/redeem"))
                .body(body),
            4096,
        )?;
        let credential = DiscoveryCredential::new(
            self.origin.clone(),
            wire.id,
            wire.client_id,
            wire.expires_at,
            wire.token,
        )?;
        if wire.scope != "discovery"
            || credential.client_id != client_id
            || !valid_expiry(credential.expires_at, now, 30 * 86_400_000)
        {
            return Err(invalid());
        }
        Ok(credential)
    }
    pub fn discover(
        &self,
        credential: &DiscoveryCredential,
        now: i64,
    ) -> io::Result<Vec<DiscoveredVault>> {
        self.admit_credential(credential, now)?;
        let response: VaultsWire = self.json(
            self.request(Method::GET, "/api/v3/sync/discovery/vaults")
                .bearer_auth(credential.expose_secret()),
            2 * 1024 * 1024,
        )?;
        let mut seen = HashSet::new();
        if response.vaults.len() > 4096
            || response.vaults.iter().any(|v| {
                v.vault_id.is_nil() || !valid_name(&v.name, 256) || !seen.insert(v.vault_id)
            })
        {
            return Err(invalid());
        }
        Ok(response.vaults)
    }
    pub fn sign_out(&self, credential: &DiscoveryCredential, now: i64) -> io::Result<()> {
        self.admit_credential(credential, now)?;
        let response: SignOutWire = self.json(
            self.request(Method::POST, "/api/v3/sync/discovery/sign-out")
                .bearer_auth(credential.expose_secret()),
            4096,
        )?;
        if !response.ok {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn restore_credential(
        &self,
        origin: &str,
        id: Uuid,
        client_id: Uuid,
        expires_at: i64,
        token: String,
    ) -> io::Result<DiscoveryCredential> {
        let token = Zeroizing::new(token);
        if origin != self.origin {
            return Err(invalid());
        }
        DiscoveryCredential::new(
            self.origin.clone(),
            id,
            client_id,
            expires_at,
            token.to_string(),
        )
    }
    fn admit_credential(&self, credential: &DiscoveryCredential, now: i64) -> io::Result<()> {
        if credential.origin() != self.origin {
            return Err(invalid());
        }
        credential_time(credential, now)
    }
    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.client
            .request(method, format!("{}{path}", self.origin))
            .header("x-lapis-protocol", "3")
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .header("cache-control", "no-store")
    }
    fn json<T: DeserializeOwned>(&self, request: RequestBuilder, max: u64) -> io::Result<T> {
        let response = request.send().map_err(|_| unavailable())?;
        if matches!(response.status().as_u16(), 401 | 403) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Sync account request is not authorized",
            ));
        }
        if !response.status().is_success() {
            return Err(unavailable());
        }
        if response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .map(str::trim)
            != Some("application/json")
        {
            return Err(invalid());
        }
        if response.content_length().is_some_and(|size| size > max) {
            return Err(invalid());
        }
        let mut bytes = Zeroizing::new(Vec::new());
        response
            .take(max + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| unavailable())?;
        if bytes.len() as u64 > max {
            return Err(invalid());
        }
        serde_json::from_slice(&bytes).map_err(|_| invalid())
    }
}
fn credential_time(credential: &DiscoveryCredential, now: i64) -> io::Result<()> {
    if now < 0 || credential.expires_at <= now {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Sync discovery credential expired",
        ));
    }
    Ok(())
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid Sync account transport value",
    )
}
fn unavailable() -> io::Error {
    io::Error::new(
        io::ErrorKind::ConnectionAborted,
        "Sync account request did not complete; inspect before retrying",
    )
}
