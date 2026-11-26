use super::{Host, Principal, Result, error};
use crate::credentials::{CredentialNamespace, CredentialStore};
use lapis_app_core::credentials::{PluginCredentialCommand, PluginCredentialReply, PluginGrants};
use lapis_app_protocol::ErrorCode;
use std::sync::Arc;
use uuid::Uuid;

pub(super) struct PluginCredentials {
    pub(super) store: Arc<CredentialStore>,
    pub(super) owner: Uuid,
    pub(super) grants: PluginGrants,
}

impl PluginCredentials {
    pub(super) fn jobs(&self) -> lapis_app_core::services::JobCredentialScope {
        let store = self.store.clone();
        let owner = self.owner;
        let grants: std::collections::HashSet<String> =
            self.grants.ids().into_iter().map(str::to_owned).collect();
        Arc::new(move |vault, plugin, job| {
            if !grants.contains(plugin) {
                return Err(lapis_app_core::credentials::CredentialError::Unavailable);
            }
            store.scope(
                owner,
                CredentialNamespace::Plugin(lapis_app_core::services::job_credential_namespace(
                    vault, plugin, job,
                )),
            )
        })
    }
}

impl Host {
    pub(crate) fn has_plugin_credential_grants(&self) -> bool {
        self.plugin_credentials
            .as_ref()
            .is_some_and(|credentials| !credentials.grants.is_empty())
    }

    pub(crate) fn plugin_credential_grants(
        &self,
        client: Uuid,
        principal: Principal,
    ) -> Result<Vec<String>> {
        self.client(client, principal)?;
        Ok(self
            .plugin_credentials
            .as_ref()
            .map(|credentials| {
                credentials
                    .grants
                    .ids()
                    .into_iter()
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default())
    }

    pub(crate) fn plugin_credentials(
        &self,
        client: Uuid,
        principal: Principal,
        command: PluginCredentialCommand,
    ) -> Result<PluginCredentialReply> {
        let client = self.client(client, principal)?;
        let credentials = self.plugin_credentials.as_ref().ok_or_else(|| {
            error(
                ErrorCode::Unsupported,
                "Plugin credential storage is unavailable",
            )
        })?;
        client
            .plugin_credentials(command, &credentials.grants, |id| {
                credentials
                    .store
                    .scope(credentials.owner, CredentialNamespace::Plugin(id))
            })
            .map_err(|error| error.0)
    }
}
