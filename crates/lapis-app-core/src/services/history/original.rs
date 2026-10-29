use super::{policy::Policy, source::Source};
use crate::services::VaultService;
use jj_history_service::{Error, Result};
use lapis_app_protocol::repository_sources::RepositorySources;
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct Grant {
    pub token: String,
    source: Source,
    pub(super) revoked: bool,
}
impl Policy {
    pub fn original(vault: &VaultService) -> Result<Option<Source>> {
        let root = {
            let state = vault.lock().map_err(|_| Error::Unavailable)?;
            state
                .root
                .verify_physical_root()
                .map_err(|_| Error::Denied)?;
            state.root.path().to_path_buf()
        };
        let source = Source::original(&root).map_err(|_| Error::Denied)?;
        if let (Some(original), Ok(Some(managed))) = (&source, Self::source(vault))
            && original.metadata == managed.metadata
        {
            return Ok(None);
        }
        vault
            .lock()
            .map_err(|_| Error::Unavailable)?
            .root
            .verify_physical_root()
            .map_err(|_| Error::Denied)?;
        Ok(source)
    }
    pub fn open_original(&self, client: Uuid) -> Result<RepositorySources> {
        let vault = self.vault.upgrade().ok_or(Error::Denied)?;
        self.vault(client, &vault.id.to_string())?;
        let source = Self::original(&vault);
        let mut grants = self.originals.lock().map_err(|_| Error::Unavailable)?;
        if let Some(grant) = grants.get_mut(&client) {
            if grant.revoked || source.as_ref().ok().and_then(Option::as_ref) != Some(&grant.source)
            {
                grant.revoked = true;
                return Err(Error::EpochChanged);
            }
            return Ok(RepositorySources {
                original: true,
                token: Some(grant.token.clone()),
            });
        }
        let source = source?.ok_or(Error::Unavailable)?;
        if grants.len() >= 16 {
            return Err(Error::Busy);
        }
        let token = format!("original:{}", Uuid::new_v4());
        grants.insert(
            client,
            Grant {
                token: token.clone(),
                source,
                revoked: false,
            },
        );
        Ok(RepositorySources {
            original: true,
            token: Some(token),
        })
    }
    pub fn resolve(
        &self,
        client: Uuid,
        requested: &str,
        vault: &VaultService,
    ) -> Result<Option<Source>> {
        if requested == vault.id.to_string() {
            return Self::source(vault);
        }
        let grant = self
            .originals
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(&client)
            .filter(|grant| grant.token == requested)
            .cloned()
            .ok_or(Error::Denied)?;
        if grant.revoked {
            return Err(Error::EpochChanged);
        }
        if grant.source.verify().is_err()
            || Self::original(vault).ok().flatten().as_ref() != Some(&grant.source)
        {
            self.revoke_original(client)?;
            return Err(Error::EpochChanged);
        }
        Ok(Some(grant.source))
    }
    pub fn revoke_original(&self, client: Uuid) -> Result<()> {
        if let Some(grant) = self
            .originals
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get_mut(&client)
        {
            grant.revoked = true;
        }
        Ok(())
    }
}
