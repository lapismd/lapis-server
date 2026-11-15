//! Host-owned encrypted token adapter. Core owns shared account orchestration.
use super::*;
use lapis_app_core::services::{NativeRemoteCredentials, RemoteTokenStore};
use lapis_app_protocol::repository_remotes::*;
use std::{path::PathBuf, sync::Arc};
struct OwnerTokens {
    store: Arc<CredentialStore>,
    owner: Uuid,
}
impl CredentialStore {
    /// All arguments are installation composition, never HTTP body fields.
    /// The owner comes from authenticated control state. Metadata roots remain
    /// separate from ciphertext; missing keys construct a visibly locked provider.
    pub fn remote_credentials(
        self: &Arc<Self>,
        owner: Uuid,
        accounts: PathBuf,
        oauth_client: Option<String>,
        ssh_available: bool,
    ) -> io::Result<Arc<NativeRemoteCredentials>> {
        self.remote_credentials_with_transport(
            owner,
            accounts,
            oauth_client,
            ssh_available,
            Arc::new(jj_forge::accounts::http::Http),
        )
    }
    pub(super) fn remote_credentials_with_transport(
        self: &Arc<Self>,
        owner: Uuid,
        accounts: PathBuf,
        oauth_client: Option<String>,
        ssh_available: bool,
        transport: Arc<dyn jj_forge::accounts::http::Transport>,
    ) -> io::Result<Arc<NativeRemoteCredentials>> {
        if owner.is_nil() {
            return Err(unavailable());
        }
        let forges = jj_forge::accounts::Manager::new(
            accounts.join(owner.to_string()),
            self.forge_backend(owner)?,
            transport,
            oauth_client,
        );
        Ok(Arc::new(NativeRemoteCredentials::new(
            RemoteHost::Server,
            Arc::new(OwnerTokens {
                store: self.clone(),
                owner,
            }),
            Arc::new(forges),
            ssh_available,
        )))
    }
}
fn identity(value: &str) -> io::Result<Uuid> {
    let id = Uuid::parse_str(value).map_err(|_| unavailable())?;
    if id.is_nil() || id.to_string() != value {
        return Err(unavailable());
    }
    Ok(id)
}
fn host(endpoint: &str) -> io::Result<String> {
    let url = url::Url::parse(endpoint).map_err(|_| unavailable())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
    {
        return Err(unavailable());
    }
    Ok(url
        .origin()
        .ascii_serialization()
        .trim_start_matches("https://")
        .into())
}
impl RemoteTokenStore for OwnerTokens {
    fn state(&self) -> RemoteStorageState {
        if self.store.available() {
            RemoteStorageState::Available
        } else {
            RemoteStorageState::Locked
        }
    }
    fn inventory(&self) -> io::Result<Vec<RemoteCredentialSummary>> {
        self.store
            .inventory(self.owner)?
            .into_iter()
            .map(|c| {
                Ok(RemoteCredentialSummary {
                    id: c.id.to_string(),
                    host: host(&c.endpoint)?,
                    label: c.label,
                    username: c.username,
                    method: RemoteCredentialMethod::Token,
                    available: true,
                })
            })
            .collect()
    }
    fn save(&self, input: &RemoteCredentialInput) -> io::Result<()> {
        if !matches!(input.provider, RemoteCredentialProvider::Https) {
            return Err(unavailable());
        }
        let endpoint = super::record::endpoint(&format!("https://{}", input.host))?;
        if let Some(id) = &input.replace {
            let id = identity(id)?;
            let saved = self
                .store
                .inventory(self.owner)?
                .into_iter()
                .find(|c| c.id == id)
                .ok_or_else(unavailable)?;
            if saved.endpoint != endpoint || saved.username != input.username {
                return Err(unavailable());
            }
            self.store
                .replace(self.owner, id, Zeroizing::new(input.token.clone()))
        } else {
            self.store
                .save(
                    self.owner,
                    TokenInput {
                        label: input.label.clone(),
                        endpoint,
                        username: input.username.clone(),
                        token: input.token.clone(),
                    },
                )
                .map(|_| ())
        }
    }
    fn forget(&self, id: &str) -> io::Result<()> {
        self.store.forget(self.owner, identity(id)?)
    }
    fn resolve(&self, connection: &RemoteConnection) -> io::Result<jj_vault::GitCredentials> {
        if connection.authentication != RemoteAuthentication::Token {
            return Err(unavailable());
        }
        let saved = self.store.resolve(
            self.owner,
            identity(&connection.credential)?,
            &connection.url,
        )?;
        jj_vault::GitCredentials::basic(&saved.username, &saved.token).map_err(|_| unavailable())
    }
}
