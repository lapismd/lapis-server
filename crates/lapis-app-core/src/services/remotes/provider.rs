//! Shared native policy; the host owns token custody and the forge manager root.
mod controls;
mod scope;
use super::*;
use jj_forge::accounts::{Authorization, Manager};
use std::{io, sync::Mutex};

/// A host injects OS or encrypted custody, already bound to its admitted owner.
/// Implementations recheck endpoint/account scope and never return saved secrets
/// through inventory. Calls execute only within the accepted native operation.
pub trait RemoteTokenStore: Send + Sync {
    fn state(&self) -> RemoteStorageState;
    fn inventory(&self) -> io::Result<Vec<RemoteCredentialSummary>>;
    fn save(&self, input: &RemoteCredentialInput) -> io::Result<()>;
    fn forget(&self, id: &str) -> io::Result<()>;
    fn resolve(&self, connection: &RemoteConnection) -> io::Result<jj_vault::GitCredentials>;
}
pub struct NativeRemoteCredentials {
    host: RemoteHost,
    tokens: Arc<dyn RemoteTokenStore>,
    forges: Arc<Manager>,
    ssh_available: bool,
    authorization: Mutex<Option<Authorization>>,
}
impl NativeRemoteCredentials {
    /// Metadata directory, credential namespace and owner come from the host,
    /// never a renderer command. The manager must use that host's secret store.
    pub fn new(
        host: RemoteHost,
        tokens: Arc<dyn RemoteTokenStore>,
        forges: Arc<Manager>,
        ssh_available: bool,
    ) -> Self {
        Self {
            host,
            tokens,
            forges,
            ssh_available,
            authorization: Mutex::new(None),
        }
    }
    fn settings(&self, storage: RemoteStorageState) -> RemoteSettings {
        RemoteSettings {
            host: self.host,
            storage,
            ssh_available: self.ssh_available,
            device_sign_in_available: matches!(storage, RemoteStorageState::Available)
                && self.forges.client_id.is_some(),
            authorization: None,
            credentials: vec![],
            targets: vec![],
            automatic_sync: false,
        }
    }
    fn saved(&self, connection: &RemoteConnection) -> Result<RemoteCredentialSummary> {
        let settings = self.inventory()?;
        if !matches!(settings.storage, RemoteStorageState::Available) {
            return Err(unavailable());
        }
        let expected = match connection.authentication {
            RemoteAuthentication::Token => RemoteCredentialMethod::Token,
            RemoteAuthentication::Forge => RemoteCredentialMethod::Forge,
            _ => return Err(invalid()),
        };
        let credential = settings
            .credentials
            .into_iter()
            .find(|c| {
                c.id == connection.credential
                    && c.available
                    && std::mem::discriminant(&c.method) == std::mem::discriminant(&expected)
            })
            .ok_or_else(invalid)?;
        scope::connection(&credential.host, &connection.url)?;
        Ok(credential)
    }
}
impl RemoteCredentials for NativeRemoteCredentials {
    fn inventory(&self) -> Result<RemoteSettings> {
        let state = self.tokens.state();
        if !matches!(state, RemoteStorageState::Available) {
            return Ok(self.settings(state));
        }
        let (Ok(mut credentials), Ok(inventory)) =
            (self.tokens.inventory(), self.forges.inventory())
        else {
            return Ok(self.settings(RemoteStorageState::Locked));
        };
        for account in inventory.accounts {
            let host = scope::account_host(&account)?;
            credentials.push(RemoteCredentialSummary {
                id: account.id,
                label: account.login.clone(),
                host,
                username: account.login,
                method: RemoteCredentialMethod::Forge,
                available: account.available,
            });
        }
        let mut ids = std::collections::HashSet::new();
        if credentials.iter().any(|c| !ids.insert(&c.id)) {
            return Err(unavailable());
        }
        let mut settings = self.settings(RemoteStorageState::Available);
        settings.credentials = credentials;
        settings.authorization = self
            .authorization
            .lock()
            .map_err(|_| unavailable())?
            .as_ref()
            .filter(|a| matches!(a.state.as_str(), "pending" | "expired"))
            .map(|a| RemoteAuthorization {
                code: a.user_code.clone(),
                url: a.verification_uri.clone(),
                state: if a.state == "expired"
                    || a.expires_at <= jj_forge::accounts::now().unwrap_or(u64::MAX)
                {
                    RemoteAuthorizationState::Expired
                } else {
                    RemoteAuthorizationState::Pending
                },
            });
        Ok(settings)
    }
    fn resolve_async<'a>(
        &'a self,
        connection: &'a RemoteConnection,
    ) -> RemoteCredentialFuture<'a, jj_vault::GitCredentials> {
        Box::pin(async move {
            let selected = self.saved(connection)?;
            if connection.authentication == RemoteAuthentication::Token {
                return self.tokens.resolve(connection).map_err(|_| unavailable());
            }
            let account = self.forges.account(&selected.id).map_err(forge_error)?;
            scope::connection(&scope::account_host(&account)?, &connection.url)?;
            if account.login != selected.username {
                return Err(invalid());
            }
            let credential = self
                .forges
                .credential(&account.id, jj_forge::accounts::now().map_err(forge_error)?)
                .await
                .map_err(forge_error)?;
            if account.api_url.ends_with("/api/v1") {
                jj_vault::GitCredentials::basic(&account.login, &credential.access_token)
            } else {
                jj_vault::GitCredentials::token(&credential.access_token)
            }
            .map_err(|_| unavailable())
        })
    }
    fn control(&self, action: RemoteCredentialAction) -> RemoteCredentialFuture<'_> {
        Box::pin(async move { self.control_native(action).await })
    }
}
fn invalid() -> Error {
    Error::new(
        ErrorCode::InvalidRequest,
        "Choose a valid scoped credential and account",
    )
}
fn unavailable() -> Error {
    Error::new(
        ErrorCode::PermissionDenied,
        "Credential storage is locked or unavailable",
    )
}
fn forge_error(error: jj_forge::Error) -> Error {
    use jj_forge::Error as E;
    let code = match error {
        E::Invalid => ErrorCode::InvalidRequest,
        E::Denied => ErrorCode::PermissionDenied,
        E::Stale => ErrorCode::Conflict,
        E::Unsupported => ErrorCode::Unsupported,
        E::Limit | E::RateLimited => ErrorCode::Busy,
        _ => ErrorCode::Io,
    };
    Error::new(
        code,
        "Forge authentication could not complete; refresh before retrying",
    )
}
