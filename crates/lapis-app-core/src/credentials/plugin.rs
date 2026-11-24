use super::{CredentialError, ScopedCredentialStore, SecretBytes, SecretName};
use lapis_app_protocol::{ErrorCode, ServiceError};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

const MAX_PLUGINS: usize = 128;

/// Administrator approval is installation policy, independent of a plugin manifest.
#[derive(Default)]
pub struct PluginGrants(HashSet<String>);

impl PluginGrants {
    pub fn parse(input: &str) -> Result<Self, CredentialError> {
        let mut grants = HashSet::new();
        for id in input.split(',').map(str::trim).filter(|id| !id.is_empty()) {
            if grants.len() >= MAX_PLUGINS || !valid_id(id) {
                return Err(CredentialError::InvalidInput);
            }
            grants.insert(id.to_owned());
        }
        Ok(Self(grants))
    }

    pub fn contains(&self, id: &str) -> bool {
        self.0.contains(id)
    }

    pub fn ids(&self) -> Vec<&str> {
        let mut ids: Vec<_> = self.0.iter().map(String::as_str).collect();
        ids.sort_unstable();
        ids
    }
}

fn valid_id(id: &str) -> bool {
    let mut bytes = id.bytes();
    id.len() <= 128
        && bytes
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginCredentialCommand {
    Open {
        plugin: String,
    },
    Get {
        lease: Uuid,
        name: String,
    },
    Set {
        lease: Uuid,
        name: String,
        value: String,
    },
    Delete {
        lease: Uuid,
        name: String,
    },
    Close {
        lease: Uuid,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginCredentialReply {
    Opened { lease: Uuid },
    Value { value: Option<String> },
    Done,
}

#[derive(Default)]
pub(crate) struct PluginLeases {
    closed: bool,
    entries: HashMap<Uuid, (String, Arc<dyn ScopedCredentialStore>)>,
}

impl PluginLeases {
    pub(crate) fn revoke(&mut self) {
        self.closed = true;
        self.entries.clear();
    }

    pub(crate) fn call(
        &mut self,
        command: PluginCredentialCommand,
        grants: &PluginGrants,
        scope: impl FnOnce(SecretName) -> Result<Arc<dyn ScopedCredentialStore>, CredentialError>,
    ) -> Result<PluginCredentialReply, ServiceError> {
        use PluginCredentialCommand as Command;
        use PluginCredentialReply as Reply;
        if self.closed {
            return Err(denied());
        }
        match command {
            Command::Open { plugin } => {
                if !grants.contains(&plugin) {
                    return Err(denied());
                }
                let store = scope(name(&plugin)?).map_err(storage)?;
                // A lost opening response can be retried without accumulating leases.
                self.entries.retain(|_, (id, _)| id != &plugin);
                let lease = Uuid::new_v4();
                self.entries.insert(lease, (plugin, store));
                Ok(Reply::Opened { lease })
            }
            Command::Get { lease, name: id } => {
                let value = self
                    .store(lease, grants)?
                    .get(&name(&id)?)
                    .map_err(storage)?;
                let value = value
                    .as_ref()
                    .map(|bytes| {
                        std::str::from_utf8(bytes.expose_secret())
                            .map(str::to_owned)
                            .map_err(|_| storage(CredentialError::Unavailable))
                    })
                    .transpose()?;
                Ok(Reply::Value { value })
            }
            Command::Set {
                lease,
                name: id,
                value,
            } => {
                let value = SecretBytes::new(value.into_bytes()).map_err(storage)?;
                self.store(lease, grants)?
                    .set(&name(&id)?, &value)
                    .map_err(storage)?;
                Ok(Reply::Done)
            }
            Command::Delete { lease, name: id } => {
                self.store(lease, grants)?
                    .delete(&name(&id)?)
                    .map_err(storage)?;
                Ok(Reply::Done)
            }
            Command::Close { lease } => {
                self.entries.remove(&lease);
                Ok(Reply::Done)
            }
        }
    }

    fn store(
        &self,
        lease: Uuid,
        grants: &PluginGrants,
    ) -> Result<&dyn ScopedCredentialStore, ServiceError> {
        self.entries
            .get(&lease)
            .filter(|(plugin, _)| grants.contains(plugin))
            .map(|(_, store)| store.as_ref())
            .ok_or_else(denied)
    }
}

fn name(id: &str) -> Result<SecretName, ServiceError> {
    if !valid_id(id) {
        return Err(storage(CredentialError::InvalidInput));
    }
    SecretName::new(id).map_err(storage)
}

fn denied() -> ServiceError {
    ServiceError {
        code: ErrorCode::PermissionDenied,
        message: "Plugin secret session is not admitted".into(),
    }
}

fn storage(error: CredentialError) -> ServiceError {
    let code = match error {
        CredentialError::InvalidInput => ErrorCode::InvalidRequest,
        CredentialError::Locked => ErrorCode::PermissionDenied,
        CredentialError::Unavailable => ErrorCode::Io,
    };
    ServiceError {
        code,
        message: error.to_string(),
    }
}
