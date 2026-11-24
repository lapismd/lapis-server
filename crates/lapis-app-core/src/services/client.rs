use super::{Error, Result, VaultService, operations};
use crate::credentials::{
    CredentialError, PluginCredentialCommand, PluginCredentialReply, PluginGrants, PluginLeases,
    ScopedCredentialStore, SecretName,
};
use lapis_app_protocol::{ClientDescriptor, ExpectedVersion, TextSnapshot};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct Client {
    pub(super) vault: Arc<VaultService>,
    pub(super) id: Uuid,
    plugin_leases: Mutex<PluginLeases>,
}
impl VaultService {
    pub fn attach(self: &Arc<Self>) -> Result<Client> {
        let mut state = self.lock()?;
        state.require_open()?;
        state.root.verify_physical_root()?;
        if state.clients.len() >= 128 {
            return Err(Error::busy());
        }
        let id = Uuid::new_v4();
        state.clients.insert(id);
        Ok(Client {
            vault: self.clone(),
            id,
            plugin_leases: Mutex::default(),
        })
    }
}
impl Client {
    pub fn plugin_credentials(
        &self,
        command: PluginCredentialCommand,
        grants: &PluginGrants,
        scope: impl FnOnce(
            SecretName,
        )
            -> std::result::Result<Arc<dyn ScopedCredentialStore>, CredentialError>,
    ) -> Result<PluginCredentialReply> {
        let mut leases = self.plugin_leases.lock().map_err(|_| Error::busy())?;
        self.vault.lock()?.require_client(self.id)?;
        let reply = leases.call(command, grants, scope).map_err(Error)?;
        self.vault.lock()?.require_client(self.id)?;
        Ok(reply)
    }

    pub fn descriptor(&self) -> ClientDescriptor {
        ClientDescriptor {
            client_id: self.id,
            vault_id: self.vault.id,
            generation: self.vault.generation,
        }
    }
    pub fn read_text(&self, path: &str) -> Result<TextSnapshot> {
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        operations::read(&state, path)
    }
    pub fn write_text(&self, path: &str, text: &str, expected: &ExpectedVersion) -> Result<String> {
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        operations::write(&state, path, text, expected, Some(self.id))
    }
}
impl Client {
    /// Native attachment retirement fences retained callers before reader drain.
    pub fn detach(&self) -> Result<()> {
        self.plugin_leases
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .revoke();
        let mut state = self.vault.state.lock().unwrap_or_else(|e| e.into_inner());
        state.clients.remove(&self.id);
        state.editors.retain(|_, editor| {
            if editor.client != self.id {
                return true;
            }
            editor.detached = true;
            editor.blocked()
        });
        state
            .retired_editor_intents
            .retain(|_, client| *client != self.id);
        let watcher = if state.clients.is_empty() {
            self.vault
                .file_changes
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
        } else {
            None
        };
        drop(state);
        drop(watcher);
        let terminals = self.vault.revoke_terminals(Some(self.id));
        let history = self.vault.revoke_history_reader(self.id);
        terminals.and(history)
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.detach();
    }
}
