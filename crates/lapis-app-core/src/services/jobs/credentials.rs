use super::*;
use crate::credentials::{CredentialError, ScopedCredentialStore, SecretBytes, SecretName};

/// Installation-owned custody. The host fixes owner identity and checks its
/// administrator grants; neither scope paths nor native identities enter IPC.
pub type CredentialScope = Arc<
    dyn Fn(Uuid, &str, &str) -> std::result::Result<Arc<dyn ScopedCredentialStore>, CredentialError>
        + Send
        + Sync,
>;

/// Stable, unambiguous backend namespace shared by host adapters.
pub fn credential_namespace(vault: Uuid, plugin: &str, job: &str) -> SecretName {
    use sha2::{Digest, Sha256};
    let identity = serde_json::to_vec(&(vault, plugin, job)).expect("String tuple serializes");
    // Colon is forbidden in plugin IDs, so a foreground plugin cannot claim
    // this namespace and alias background credential custody.
    SecretName::new(format!("background-job:{:x}", Sha256::digest(identity)))
        .expect("Digest is a valid name")
}

impl broker::Broker {
    pub(super) fn credential_store(&self) -> Result<Arc<dyn ScopedCredentialStore>> {
        let stored = self.owner.store()?;
        let job = stored
            .catalog
            .jobs
            .get(self.index)
            .ok_or_else(Error::denied)?;
        scope(
            &self.owner,
            self.work.vault.id,
            &job.plugin,
            &job.descriptor.id,
        )
    }
}

fn scope(
    owner: &Owner,
    vault: Uuid,
    plugin: &str,
    job: &str,
) -> Result<Arc<dyn ScopedCredentialStore>> {
    owner
        .credential_scope
        .as_ref()
        .ok_or_else(|| invalid("Job credential custody unavailable"))?(vault, plugin, job)
    .map_err(|_| invalid("Job credential custody unavailable or locked"))
}

pub(super) fn statuses(
    owner: &Owner,
    vault: Uuid,
    plugin: &str,
    descriptor: &JobDescriptor,
) -> Option<Vec<JobCredentialStatus>> {
    let mut ids = std::collections::BTreeSet::new();
    for grant in descriptor
        .permissions
        .network
        .as_deref()
        .unwrap_or_default()
    {
        ids.extend(grant.credentials.iter().map(|binding| binding.id.clone()));
    }
    if ids.is_empty() {
        return None;
    }
    Some(
        ids.into_iter()
            .map(|id| {
                let result = scope(owner, vault, plugin, &descriptor.id).and_then(|store| {
                    store
                        .get(&SecretName::new(id.clone()).map_err(|_| Error::denied())?)
                        .map(|secret| secret.is_some())
                        .map_err(|_| invalid("Job credential unavailable or locked"))
                });
                JobCredentialStatus {
                    id,
                    configured: matches!(&result, Ok(true)),
                    unavailable_reason: result.err().map(|error| error.to_string()),
                }
            })
            .collect(),
    )
}

pub(super) fn store(
    vault: &VaultService,
    owner: &Owner,
    plugin: &str,
    job: &str,
    id: &str,
    value: Option<String>,
) -> Result<()> {
    // Clear the incoming owned allocation at the admitted use site.
    let value = value
        .map(|value| SecretBytes::new(value.into_bytes()))
        .transpose()
        .map_err(|_| invalid("Job credential exceeds bounds"))?;
    let installed = discovery::discover(vault)?
        .into_iter()
        .find(|entry| entry.plugin == plugin && entry.descriptor.id == job)
        .ok_or_else(Error::denied)?;
    let stored = owner.store()?;
    if owner.stopped.load(Ordering::Acquire)
        || !stored.catalog.trusted
        || stored.catalog.safe_mode
        || !installed
            .descriptor
            .permissions
            .network
            .as_deref()
            .unwrap_or_default()
            .iter()
            .any(|grant| grant.credentials.iter().any(|binding| binding.id == id))
    {
        return Err(Error::denied());
    }
    let name = SecretName::new(id.to_owned()).map_err(|_| Error::denied())?;
    if stored.catalog.jobs.iter().any(|entry| {
        entry.plugin == plugin && entry.descriptor.id == job && super::control::active(entry)
    }) {
        owner.cancelled.store(true, Ordering::Release);
    }
    let store = scope(owner, vault.id, plugin, job)?;
    if let Some(value) = value {
        store.set(&name, &value)
    } else {
        store.delete(&name)
    }
    .map_err(|_| invalid("Job credential replacement failed"))?;
    drop(stored);
    owner.store()?.update(|_| Ok(()))
}
