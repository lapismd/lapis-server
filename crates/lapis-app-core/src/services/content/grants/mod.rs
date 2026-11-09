//! Process-local native content grants. Hosts own consent and all management calls;
//! transports receive only bearer dispatch, never issuance or filesystem authority.
mod access;
#[cfg(test)]
mod access_tests;
mod issuance;
mod lifecycle;
#[cfg(test)]
mod tests;
mod token;

use super::{ContentSession, ContentVault, Result, failure};
use lapis_app_protocol::content::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
};
use subtle::ConstantTimeEq;
pub use token::{ContentToken, IssuedContentGrant};
use uuid::Uuid;

pub const MAX_CONTENT_GRANTS: usize = 128;

/// Native owner handle, not a wire DTO. Do not expose its management methods
/// through bearer-authenticated content routes. All methods can block.
#[derive(Default)]
pub struct ContentGrants {
    registry: Mutex<Registry>,
    transition: Mutex<()>,
}

#[derive(Default)]
struct Registry {
    entries: BTreeMap<Uuid, Arc<Entry>>,
    closed: bool,
    paused: bool,
    // Held by in-flight issuance: replacing this identity prevents stale publication.
    epoch: Arc<()>,
}

struct Entry {
    verifier: [u8; 32],
    session: ContentSession,
    label: String,
    vault_ids: Vec<Uuid>,
}

fn unauthenticated() -> ContentError {
    failure(
        ContentErrorCode::Unauthenticated,
        "Invalid content credential",
    )
}

impl ContentGrants {
    fn lock(&self) -> Result<MutexGuard<'_, Registry>> {
        self.registry
            .lock()
            .map_err(|_| failure(ContentErrorCode::Unavailable, "Content grants unavailable"))
    }

    // Do not expose the Arc/session: every call must verify the original token.
    fn authenticate(&self, bearer: &str) -> Result<Arc<Entry>> {
        let verifier = token::verifier(bearer).ok_or_else(unauthenticated)?;
        let registry = self.lock()?;
        if registry.closed || registry.paused {
            return Err(unauthenticated());
        }
        let entry = registry
            .entries
            .values()
            .find(|entry| bool::from(entry.verifier.ct_eq(&verifier)))
            .ok_or_else(unauthenticated)?;
        entry
            .session
            .require_live()
            .map_err(|_| unauthenticated())?;
        Ok(Arc::clone(entry))
    }

    /// Credential preflight for adapters before decoding operation input.
    /// This returns no authority handle; `execute` always authenticates again.
    pub fn check_credential(&self, bearer: &str) -> Result<()> {
        self.authenticate(bearer).map(|_| ())
    }

    /// Verify a credential and dispatch through its original scoped session.
    /// Authorization is rechecked by that session after every native wait.
    pub fn execute(&self, bearer: &str, request: ContentRequest) -> ContentReply {
        match self.authenticate(bearer) {
            Ok(entry) => {
                let mut reply = entry.session.execute(request);
                if let ContentOutcome::Error { error } = &mut reply.outcome
                    && error.code == ContentErrorCode::Unauthenticated
                {
                    *error = unauthenticated();
                }
                reply
            }
            Err(error) => ContentReply {
                api_version: CONTENT_API_MAJOR,
                request_id: request.request_id,
                outcome: ContentOutcome::Error { error },
            },
        }
    }
}

impl Registry {
    fn require_capacity(&self) -> Result<()> {
        if self.closed || self.paused {
            return Err(failure(
                ContentErrorCode::Unavailable,
                "Content access is disabled",
            ));
        }
        if self.entries.len() >= MAX_CONTENT_GRANTS {
            return Err(failure(
                ContentErrorCode::Busy,
                "Content grant limit reached",
            ));
        }
        Ok(())
    }
}
