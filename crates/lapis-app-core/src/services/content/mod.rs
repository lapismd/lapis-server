//! Native read-only content sessions. Hosts authenticate before constructing one
//! from dedicated admitted clients; no HTTP credential or caller-selected root enters here.
mod grants;
mod pagination;
mod plugin_operations;
mod reads;
pub use grants::{ContentGrants, ContentToken, IssuedContentGrant, MAX_CONTENT_GRANTS};
#[cfg(test)]
mod tests;

use super::Client;
use lapis_app_protocol::content::*;
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

pub struct ContentVault {
    pub name: String,
    pub client: Client,
}

pub struct ContentSession {
    state: Mutex<State>,
    revoked: AtomicBool,
    expires: Instant,
}

struct State {
    vaults: BTreeMap<Uuid, ContentVault>,
    cursors: pagination::Cursors,
}

type Result<T> = std::result::Result<T, ContentError>;

fn failure(code: ContentErrorCode, message: &str) -> ContentError {
    ContentError {
        code,
        message: message.into(),
        effect: ContentEffectStatus::NotStarted,
    }
}

impl ContentSession {
    /// Consume dedicated native attachments into a bounded, revocable read grant.
    /// Hosts retain credential verification and persisted consent/grant policy.
    pub fn read_only(vaults: Vec<ContentVault>, lifetime: Duration) -> Result<Self> {
        if vaults.len() > 32 || lifetime.is_zero() || lifetime > Duration::from_secs(86_400) {
            return Err(failure(
                ContentErrorCode::InvalidRequest,
                "Invalid content session limits",
            ));
        }
        let expires = Instant::now()
            .checked_add(lifetime)
            .ok_or_else(|| failure(ContentErrorCode::InvalidRequest, "Invalid session lifetime"))?;
        let mut admitted = BTreeMap::new();
        for vault in vaults {
            if vault.name.trim().is_empty()
                || vault.name.len() > 256
                || vault.name.chars().any(char::is_control)
            {
                return Err(failure(
                    ContentErrorCode::InvalidRequest,
                    "Invalid vault display name",
                ));
            }
            let id = vault.client.descriptor().vault_id;
            {
                let state = vault.client.vault.lock().map_err(reads::native)?;
                state
                    .require_client(vault.client.id)
                    .map_err(reads::native)?;
                state.root.verify_physical_root().map_err(reads::io)?;
            }
            if admitted.insert(id, vault).is_some() {
                return Err(failure(
                    ContentErrorCode::InvalidRequest,
                    "Duplicate content vault",
                ));
            }
        }
        Ok(Self {
            state: Mutex::new(State {
                vaults: admitted,
                cursors: Default::default(),
            }),
            revoked: AtomicBool::new(false),
            expires,
        })
    }

    fn require_live(&self) -> Result<()> {
        if self.revoked.load(Ordering::Acquire) || Instant::now() >= self.expires {
            return Err(failure(
                ContentErrorCode::Unauthenticated,
                "Content access is no longer valid",
            ));
        }
        Ok(())
    }

    /// Fence first, then drain active readers and retire the original attachments.
    /// Async hosts must call this blocking operation outside their async executor.
    pub fn revoke(&self) -> Result<()> {
        self.revoked.store(true, Ordering::Release);
        let mut state = self
            .state
            .lock()
            .map_err(|_| failure(ContentErrorCode::Unavailable, "Content session unavailable"))?;
        state.cursors = Default::default();
        state.vaults.clear();
        Ok(())
    }

    /// Blocking read dispatch. Revocation and expiry are rechecked after native waits.
    pub fn execute(&self, request: ContentRequest) -> ContentReply {
        let outcome = match self.dispatch(&request) {
            Ok(value) => ContentOutcome::Ok { value },
            Err(error) => ContentOutcome::Error { error },
        };
        ContentReply {
            api_version: CONTENT_API_MAJOR,
            request_id: request.request_id,
            outcome,
        }
    }

    fn dispatch(&self, request: &ContentRequest) -> Result<ContentValue> {
        self.require_live()?;
        request.validate()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| failure(ContentErrorCode::Unavailable, "Content session unavailable"))?;
        self.require_live()?;
        let State { vaults, cursors } = &mut *state;
        match &request.command {
            ContentCommand::Capabilities {} => Ok(ContentValue::Capabilities {
                capabilities: ContentCapabilities {
                    api_version: CONTENT_API_MAJOR,
                    operations: [
                        ContentOperation::Capabilities,
                        ContentOperation::ListVaults,
                        ContentOperation::ListNotes,
                        ContentOperation::ReadNote,
                    ]
                    .into_iter()
                    .map(ContentOperation::descriptor)
                    .collect(),
                },
            }),
            ContentCommand::ListVaults { page } => {
                let mut items = Vec::new();
                for (id, vault) in vaults.iter() {
                    let native = vault.client.vault.lock().map_err(reads::native)?;
                    self.require_live()?;
                    native
                        .require_client(vault.client.id)
                        .map_err(reads::native)?;
                    native.root.verify_physical_root().map_err(reads::io)?;
                    items.push(ContentVaultSummary {
                        vault_id: *id,
                        name: vault.name.clone(),
                    });
                }
                let (items, next_cursor) = cursors.page(pagination::Key::Vaults, items, page)?;
                Ok(ContentValue::Vaults { items, next_cursor })
            }
            ContentCommand::ReadNote { vault_id, path } => {
                let vault = vaults.get(vault_id).ok_or_else(|| {
                    failure(ContentErrorCode::PermissionDenied, "Vault is not admitted")
                })?;
                reads::read(self, &vault.client, path)
            }
            ContentCommand::ListNotes {
                vault_id,
                directory,
                page,
            } => {
                let vault = vaults.get(vault_id).ok_or_else(|| {
                    failure(ContentErrorCode::PermissionDenied, "Vault is not admitted")
                })?;
                let items = reads::list(self, &vault.client, directory.as_deref().unwrap_or(""))?;
                let key = pagination::Key::Notes {
                    vault: *vault_id,
                    directory: directory.clone(),
                };
                let (items, next_cursor) = cursors.page(key, items, page)?;
                Ok(ContentValue::Notes { items, next_cursor })
            }
            ContentCommand::CreateNote { .. } | ContentCommand::ReplaceNote { .. } => Err(failure(
                ContentErrorCode::PermissionDenied,
                "Content session is read-only",
            )),
        }
    }
}
