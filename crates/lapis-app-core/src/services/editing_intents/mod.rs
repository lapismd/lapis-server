use super::{
    Client, Error, Result,
    editors::owned,
    operations,
    state::{EditingIntent, Editor, State},
};
use lapis_app_protocol::{
    ErrorCode, ExpectedVersion,
    editing_intents::{EditingIntentCommand, EditingIntentState, EditingIntentValue},
};
use uuid::Uuid;

mod recovery;

const MAX_CLIENT_IDENTITIES: usize = 16_384;

pub(super) fn can_admit(state: &State, client: Uuid) -> bool {
    let retired = state
        .retired_editor_intents
        .values()
        .filter(|owner| **owner == client)
        .count();
    let live = state
        .editors
        .values()
        .filter(|editor| editor.client == client && editor.intent.is_some())
        .count();
    retired + live < MAX_CLIENT_IDENTITIES
}

fn conflict() -> Error {
    Error::new(ErrorCode::Conflict, "Editor intent generation changed")
}
fn snapshot(id: Uuid, editor: &Editor) -> Result<EditingIntentState> {
    let intent = editor.intent.as_ref().ok_or_else(Error::denied)?;
    Ok(EditingIntentState {
        editor_id: id,
        path: editor.path.clone(),
        generation: intent.generation,
        revision: editor.revision,
        dirty: editor.dirty,
        active: intent.active,
        detached: editor.detached,
    })
}
fn state_value(id: Uuid, editor: &Editor) -> Result<EditingIntentValue> {
    Ok(EditingIntentValue::State {
        state: snapshot(id, editor)?,
    })
}
fn generation(editor: &Editor, requested: u32) -> Result<()> {
    if requested == 0 || editor.intent.as_ref().ok_or_else(Error::denied)?.generation != requested {
        return Err(conflict());
    }
    Ok(())
}
fn verify_saved(state: &State, path: &str, expected: &ExpectedVersion) -> Result<()> {
    state.root.verify_physical_root()?;
    match expected {
        ExpectedVersion::Version { version } => {
            if operations::read(state, path)?.version == *version {
                return Ok(());
            }
        }
        ExpectedVersion::Missing {} => match state.root.entry_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        },
    }
    Err(Error::new(
        ErrorCode::Conflict,
        "Saved editor snapshot changed",
    ))
}

impl Client {
    pub fn editing_intents(&self, command: EditingIntentCommand) -> Result<EditingIntentValue> {
        let mut state = self.vault.lock()?;
        state.require_client(self.id)?;
        state.root.verify_physical_root()?;
        match command {
            EditingIntentCommand::Acquire { editor_id, path } => {
                if editor_id.is_nil() {
                    return Err(Error::new(
                        ErrorCode::InvalidRequest,
                        "Editor identity is required",
                    ));
                }
                state.root.validate_path(&path, false)?;
                if state.retired_editor_intents.contains_key(&editor_id) {
                    return Err(Error::closed());
                }
                if state.editors.contains_key(&editor_id) {
                    let editor = owned(&mut state, self.id, editor_id)?;
                    if editor.path != path {
                        return Err(Error::denied());
                    }
                    return state_value(editor_id, editor);
                }
                if state.editors.len() >= 4096 || !can_admit(&state, self.id) {
                    return Err(Error::busy());
                }
                state.editors.insert(
                    editor_id,
                    Editor {
                        client: self.id,
                        path,
                        dirty: false,
                        revision: 0,
                        detached: false,
                        intent: Some(EditingIntent {
                            generation: 1,
                            active: true,
                        }),
                    },
                );
                state_value(editor_id, &state.editors[&editor_id])
            }
            EditingIntentCommand::Changed {
                editor_id,
                generation: requested,
                revision,
            } => {
                let editor = owned(&mut state, self.id, editor_id)?;
                generation(editor, requested)?;
                if !editor.intent.as_ref().ok_or_else(Error::denied)?.active
                    || revision == 0
                    || revision < editor.revision
                    || (revision == editor.revision && !editor.dirty)
                {
                    return Err(conflict());
                }
                editor.revision = revision;
                editor.dirty = true;
                state_value(editor_id, editor)
            }
            EditingIntentCommand::Yield {
                editor_id,
                generation: requested,
                revision,
                expected,
            } => {
                let editor = owned(&mut state, self.id, editor_id)?;
                generation(editor, requested)?;
                if revision != editor.revision {
                    return Err(conflict());
                }
                // A repeat observes the prior handoff without revalidating bytes
                // that another admitted writer may already have replaced.
                if !editor.intent.as_ref().ok_or_else(Error::denied)?.active && !editor.dirty {
                    return state_value(editor_id, editor);
                }
                let path = editor.path.clone();
                verify_saved(&state, &path, &expected)?;
                let editor = owned(&mut state, self.id, editor_id)?;
                editor.intent.as_mut().ok_or_else(Error::denied)?.active = false;
                editor.dirty = false;
                state_value(editor_id, editor)
            }
            EditingIntentCommand::Resume {
                editor_id,
                generation: requested,
            } => {
                let editor = owned(&mut state, self.id, editor_id)?;
                let dirty = editor.dirty;
                let intent = editor.intent.as_mut().ok_or_else(Error::denied)?;
                if requested == intent.generation && intent.active {
                    return state_value(editor_id, editor);
                }
                if intent.active || dirty || intent.generation.checked_add(1) != Some(requested) {
                    return Err(conflict());
                }
                intent.generation = requested;
                intent.active = true;
                state_value(editor_id, editor)
            }
            EditingIntentCommand::Retire {
                editor_id,
                generation: requested,
            } => {
                if state.retired_editor_intents.get(&editor_id) == Some(&self.id) {
                    return Ok(EditingIntentValue::Retired {});
                }
                let editor = owned(&mut state, self.id, editor_id)?;
                generation(editor, requested)?;
                if editor.blocked() {
                    return Err(Error::busy());
                }
                state.editors.remove(&editor_id);
                state.retired_editor_intents.insert(editor_id, self.id);
                Ok(EditingIntentValue::Retired {})
            }
            EditingIntentCommand::Recover {
                editor_id,
                generation,
                revision,
            } => recovery::recover(&mut state, self.id, editor_id, generation, revision),
            EditingIntentCommand::Discard {
                editor_id,
                generation,
                revision,
            } => recovery::discard(&mut state, self.id, editor_id, generation, revision),
            EditingIntentCommand::Inspect { editor_id } => {
                if state.editors.contains_key(&editor_id) {
                    return state_value(editor_id, owned(&mut state, self.id, editor_id)?);
                }
                match state.retired_editor_intents.get(&editor_id) {
                    Some(client) if *client == self.id => Ok(EditingIntentValue::Retired {}),
                    Some(_) => Err(Error::denied()),
                    None => Ok(EditingIntentValue::Missing {}),
                }
            }
            EditingIntentCommand::List {} => {
                let mut states = state
                    .editors
                    .iter()
                    .filter(|(_, editor)| editor.intent.is_some() && editor.blocked())
                    .map(|(id, editor)| snapshot(*id, editor))
                    .collect::<Result<Vec<_>>>()?;
                states.sort_by(|a, b| (&a.path, a.editor_id).cmp(&(&b.path, b.editor_id)));
                Ok(EditingIntentValue::States { states })
            }
        }
    }
}
