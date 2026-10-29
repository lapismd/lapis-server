use super::{
    Client, Error, Result, operations,
    state::{Editor, State},
};
use lapis_app_protocol::{EditorClaim, ErrorCode};
use uuid::Uuid;

pub(super) fn owned(state: &mut State, client: Uuid, id: Uuid) -> Result<&mut Editor> {
    state.require_client(client)?;
    state
        .editors
        .get_mut(&id)
        .filter(|editor| editor.client == client && !editor.detached)
        .ok_or_else(Error::denied)
}
impl Client {
    pub fn editor_claims(&self) -> Result<Vec<EditorClaim>> {
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        let mut claims: Vec<_> = state
            .editors
            .iter()
            .filter(|(_, e)| e.dirty)
            .map(|(id, e)| EditorClaim {
                editor_id: *id,
                path: e.path.clone(),
                revision: e.revision,
                detached: e.detached,
            })
            .collect();
        claims.sort_by(|a, b| (&a.path, a.editor_id).cmp(&(&b.path, b.editor_id)));
        Ok(claims)
    }

    pub fn open_editor(&self, path: &str) -> Result<Uuid> {
        let mut state = self.vault.lock()?;
        state.require_client(self.id)?;
        state.root.validate_path(path, false)?;
        if state.editors.len() >= 4096 {
            return Err(Error::busy());
        }
        let id = Uuid::new_v4();
        state.editors.insert(
            id,
            Editor {
                client: self.id,
                path: path.into(),
                dirty: false,
                revision: 0,
                detached: false,
                intent: None,
            },
        );
        Ok(id)
    }
    pub fn mark_dirty(&self, id: Uuid, revision: u32) -> Result<()> {
        let mut state = self.vault.lock()?;
        let editor = owned(&mut state, self.id, id)?;
        if revision <= editor.revision {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Editor revision must advance",
            ));
        }
        editor.revision = revision;
        editor.dirty = true;
        Ok(())
    }
    pub fn mark_saved(&self, id: Uuid, revision: u32, version: &str) -> Result<bool> {
        let mut state = self.vault.lock()?;
        let editor = owned(&mut state, self.id, id)?;
        if editor.revision != revision {
            return Ok(false);
        }
        let path = editor.path.clone();
        if operations::read(&state, &path)?.version != version {
            return Err(Error::new(ErrorCode::Conflict, "Saved file changed"));
        }
        owned(&mut state, self.id, id)?.dirty = false;
        Ok(true)
    }
    pub fn close_editor(&self, id: Uuid) -> Result<()> {
        let mut state = self.vault.lock()?;
        if owned(&mut state, self.id, id)?.blocked() {
            return Err(Error::busy());
        }
        if state
            .editors
            .remove(&id)
            .is_some_and(|editor| editor.intent.is_some())
        {
            state.retired_editor_intents.insert(id, self.id);
        }
        Ok(())
    }
    /// An explicit user discard, not a disconnect or timeout.
    pub fn discard_editor(&self, id: Uuid) -> Result<()> {
        let mut state = self.vault.lock()?;
        if owned(&mut state, self.id, id)?.intent.is_some() {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Editor intents require captured-state discard",
            ));
        }
        state.editors.remove(&id);
        Ok(())
    }
    pub fn recover_editor(&self, id: Uuid) -> Result<()> {
        let mut state = self.vault.lock()?;
        state.require_client(self.id)?;
        if state
            .editors
            .get(&id)
            .is_some_and(|editor| editor.intent.is_some())
        {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Editor intents require captured-state recovery",
            ));
        }
        let editor = state.editors.get_mut(&id).ok_or_else(Error::denied)?;
        if !editor.detached {
            return Err(Error::busy());
        }
        editor.client = self.id;
        editor.detached = false;
        Ok(())
    }
}
