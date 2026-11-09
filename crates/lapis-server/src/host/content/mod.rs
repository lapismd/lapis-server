mod lifetime;
use super::{Host, Result, error};
use lapis_app_core::services::content::{ContentGrants, ContentVault, IssuedContentGrant};
use lapis_app_protocol::{
    ErrorCode, ServiceError,
    content::{ContentError, ContentErrorCode, ContentGrantRequest},
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::Duration,
};
use tokio_util::sync::CancellationToken;

pub(crate) struct Access {
    pub grants: Arc<ContentGrants>,
    pub cancellation: CancellationToken,
    pub gate: tokio::sync::Mutex<()>,
    pub admission: Mutex<()>,
    lifetime: Mutex<(bool, usize)>,
    drained: Condvar,
}
impl Default for Access {
    fn default() -> Self {
        Self {
            grants: Arc::new(ContentGrants::disabled()),
            cancellation: CancellationToken::new(),
            gate: tokio::sync::Mutex::new(()),
            admission: Mutex::new(()),
            lifetime: Mutex::new((false, 0)),
            drained: Condvar::new(),
        }
    }
}
impl Access {
    pub fn admission(&self) -> Result<MutexGuard<'_, ()>> {
        self.admission
            .lock()
            .map_err(|_| error(ErrorCode::Closed, "Content admission is unavailable"))
    }
}

impl Host {
    pub(crate) fn issue_content(&self, input: ContentGrantRequest) -> Result<IssuedContentGrant> {
        if input.vault_ids.is_empty()
            || input.vault_ids.len() > 20
            || !(1..=86_400).contains(&input.lifetime_seconds)
            || input.vault_ids.iter().any(uuid::Uuid::is_nil)
            || input.vault_ids.iter().collect::<BTreeSet<_>>().len() != input.vault_ids.len()
        {
            return Err(error(
                ErrorCode::InvalidRequest,
                "Select 1 to 20 distinct active vaults and a lifetime of 1 to 86400 seconds",
            ));
        }
        let _admission = self.content.admission()?;
        if !self.ready() || !self.content.grants.is_enabled().map_err(native)? {
            return Err(error(ErrorCode::Closed, "Content access is disabled"));
        }
        let managed = self
            .management
            .get()
            .map(|manager| manager.catalogue.content_admission(&input.vault_ids))
            .transpose()?;
        let catalogue = managed
            .as_ref()
            .map(|(_, vaults)| vaults)
            .unwrap_or(&self.vaults);
        let vaults = input
            .vault_ids
            .iter()
            .map(|id| {
                let vault = catalogue
                    .iter()
                    .find(|v| v.vault_id == *id && v.archived_at.is_none())
                    .ok_or_else(|| error(ErrorCode::PermissionDenied, "Vault is not admitted"))?;
                let client = self
                    .runtime
                    .get(*id)
                    .and_then(|owner| owner.attach())
                    .map_err(|e| e.0)?;
                Ok(ContentVault {
                    name: vault.name.clone(),
                    client,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.content
            .grants
            .issue_named_read_only(
                &input.label,
                vaults,
                Duration::from_secs(input.lifetime_seconds.into()),
            )
            .map_err(native)
    }
}

pub(crate) fn native(value: ContentError) -> ServiceError {
    let code = match value.code {
        ContentErrorCode::InvalidRequest | ContentErrorCode::TooLarge => ErrorCode::InvalidRequest,
        ContentErrorCode::PermissionDenied | ContentErrorCode::Unauthenticated => {
            ErrorCode::PermissionDenied
        }
        ContentErrorCode::NotFound => ErrorCode::NotFound,
        ContentErrorCode::Busy => ErrorCode::Busy,
        ContentErrorCode::Conflict => ErrorCode::Conflict,
        ContentErrorCode::Unsupported => ErrorCode::Unsupported,
        _ => ErrorCode::Io,
    };
    error(code, &value.message)
}
