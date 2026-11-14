use super::*;
use std::time::Duration;

impl ContentGrants {
    /// Low-level fixture issuance; production hosts use named issuance and
    /// `disabled()` at startup. The caller authorizes the exact scope/lifetime.
    pub fn issue_read_only(
        &self,
        vaults: Vec<ContentVault>,
        lifetime: Duration,
    ) -> Result<IssuedContentGrant> {
        self.issue(String::new(), vaults, lifetime, &[])
    }

    /// Owner-only issuance from dedicated host-admitted clients. A label is
    /// display data, never a credential identity or authorization input.
    pub fn issue_named_read_only(
        &self,
        label: &str,
        vaults: Vec<ContentVault>,
        lifetime: Duration,
    ) -> Result<IssuedContentGrant> {
        let control = label.chars().any(char::is_control);
        let label = label.trim();
        if label.is_empty() || label.len() > 128 || control || vaults.is_empty() {
            return Err(failure(
                ContentErrorCode::InvalidRequest,
                "Invalid content grant label or scope",
            ));
        }
        self.issue_named_operations(label, vaults, lifetime, &[])
    }

    pub fn issue_named_operations(
        &self,
        label: &str,
        vaults: Vec<ContentVault>,
        lifetime: Duration,
        permissions: &[String],
    ) -> Result<IssuedContentGrant> {
        if label.trim().is_empty()
            || label.len() > 128
            || label.chars().any(char::is_control)
            || vaults.is_empty()
        {
            return Err(failure(
                ContentErrorCode::InvalidRequest,
                "Invalid content grant label or scope",
            ));
        }
        self.issue(label.trim().into(), vaults, lifetime, permissions)
    }

    fn issue(
        &self,
        label: String,
        vaults: Vec<ContentVault>,
        lifetime: Duration,
        permissions: &[String],
    ) -> Result<IssuedContentGrant> {
        let operations = self.operations.snapshot(permissions)?;
        let epoch = {
            let registry = self.lock()?;
            registry.require_capacity()?;
            Arc::clone(&registry.epoch)
        };
        let mut vault_ids: Vec<_> = vaults
            .iter()
            .map(|v| v.client.descriptor().vault_id)
            .collect();
        vault_ids.sort_unstable();
        let session = ContentSession::read_only(vaults, lifetime)?;
        let token = ContentToken::generate()?;
        let verifier = token::verifier(token.expose_secret()).ok_or_else(unauthenticated)?;
        let grant_id = Uuid::new_v4();
        let mut registry = self.lock()?;
        registry.require_capacity()?;
        if !Arc::ptr_eq(&registry.epoch, &epoch) {
            return Err(failure(
                ContentErrorCode::Unavailable,
                "Content admission changed",
            ));
        }
        session.require_live()?;
        if registry.entries.contains_key(&grant_id)
            || registry
                .entries
                .values()
                .any(|entry| bool::from(entry.verifier.ct_eq(&verifier)))
        {
            return Err(failure(
                ContentErrorCode::Unavailable,
                "Content credential collision",
            ));
        }
        registry.entries.insert(
            grant_id,
            Arc::new(Entry {
                operations,
                verifier,
                session,
                label,
                vault_ids,
            }),
        );
        Ok(IssuedContentGrant { grant_id, token })
    }
}
