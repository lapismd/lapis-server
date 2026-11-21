use super::*;
use std::time::Duration;

/// A server-approved grant with a persisted verifier and a fixed operation snapshot.
/// The native registry owns only the short-lived lease built from this approval.
pub struct PersistentGrant<'a> {
    pub id: Uuid,
    pub label: &'a str,
    pub bearer: &'a str,
    pub permissions: &'a [String],
    pub operations: BTreeMap<String, String>,
}

impl ContentGrants {
    pub fn lease(
        &self,
        approval: PersistentGrant<'_>,
        vaults: Vec<ContentVault>,
        lifetime: Duration,
    ) -> Result<()> {
        let verifier = token::verifier(approval.bearer).ok_or_else(unauthenticated)?;
        let epoch = {
            let registry = self.lock()?;
            registry.require_capacity()?;
            Arc::clone(&registry.epoch)
        };
        let vault_ids = vaults
            .iter()
            .map(|v| v.client.descriptor().vault_id)
            .collect();
        let session = ContentSession::read_only(vaults, lifetime)?;
        let mut registry = self.lock()?;
        registry.require_capacity()?;
        if !Arc::ptr_eq(&registry.epoch, &epoch)
            || registry.entries.contains_key(&approval.id)
            || registry
                .entries
                .values()
                .any(|entry| bool::from(entry.verifier.ct_eq(&verifier)))
        {
            return Err(failure(
                ContentErrorCode::Unavailable,
                "Content admission changed",
            ));
        }
        registry.entries.insert(
            approval.id,
            Arc::new(Entry {
                permissions: approval.permissions.to_vec(),
                operations: approval.operations,
                verifier,
                session,
                label: approval.label.into(),
                vault_ids,
                durable: true,
            }),
        );
        Ok(())
    }
}
