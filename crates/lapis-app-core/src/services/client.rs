use super::{Error, Result, VaultService, operations};
use lapis_app_protocol::{ClientDescriptor, ExpectedVersion, TextSnapshot};
use std::sync::Arc;
use uuid::Uuid;

pub struct Client {
    pub(super) vault: Arc<VaultService>,
    pub(super) id: Uuid,
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
        })
    }
}
impl Client {
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
        drop(state);
        self.vault.revoke_history_reader(self.id)
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.detach();
    }
}
