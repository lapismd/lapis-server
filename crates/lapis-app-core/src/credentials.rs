//! Native secret persistence. Hosts bind each store to an admitted scope before lending it to a caller.

use std::{error::Error, fmt};
use zeroize::Zeroizing;

mod plugin;
pub(crate) use plugin::PluginLeases;
pub use plugin::{PluginCredentialCommand, PluginCredentialReply, PluginGrants};

pub const MAX_SECRET_BYTES: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialError {
    InvalidInput,
    Locked,
    Unavailable,
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "Invalid credential input",
            Self::Locked => "Credential store is locked",
            Self::Unavailable => "Credential store is unavailable",
        })
    }
}

impl Error for CredentialError {}

#[derive(Clone, Eq, PartialEq)]
pub struct SecretName(String);

impl SecretName {
    pub fn new(name: impl Into<String>) -> Result<Self, CredentialError> {
        let name = name.into();
        if name.is_empty() || name.len() > 512 || name.chars().any(char::is_control) {
            return Err(CredentialError::InvalidInput);
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque bytes that are cleared on drop. Expose them only at an admitted use site.
pub struct SecretBytes(Zeroizing<Vec<u8>>);

impl SecretBytes {
    pub fn new(bytes: Vec<u8>) -> Result<Self, CredentialError> {
        let bytes = Zeroizing::new(bytes);
        if bytes.len() > MAX_SECRET_BYTES {
            return Err(CredentialError::InvalidInput);
        }
        Ok(Self(bytes))
    }

    pub fn expose_secret(&self) -> &[u8] {
        &self.0
    }
}

/// One host-assigned installation/profile, owner and namespace. Implementations
/// must serialize or atomically publish replacements within that scope.
pub trait ScopedCredentialStore: Send + Sync {
    /// `Ok(None)` means the entry is absent in an accessible backend.
    fn get(&self, name: &SecretName) -> Result<Option<SecretBytes>, CredentialError>;

    fn set(&self, name: &SecretName, value: &SecretBytes) -> Result<(), CredentialError>;

    /// Deleting an absent entry from an accessible backend succeeds.
    fn delete(&self, name: &SecretName) -> Result<(), CredentialError>;
}
