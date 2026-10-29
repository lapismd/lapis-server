use super::{Error, Result, State, can_admit, conflict, generation, state_value};
use lapis_app_protocol::editing_intents::EditingIntentValue;
use uuid::Uuid;

pub(super) fn recover(
    state: &mut State,
    client: Uuid,
    id: Uuid,
    requested_generation: u32,
    revision: u32,
) -> Result<EditingIntentValue> {
    let editor = state.editors.get(&id).ok_or_else(Error::denied)?;
    generation(editor, requested_generation)?;
    if editor.revision != revision {
        return Err(conflict());
    }
    if !editor.detached || !can_admit(state, client) {
        return Err(Error::busy());
    }
    let next = requested_generation.checked_add(1).ok_or_else(conflict)?;
    let editor = state.editors.get_mut(&id).ok_or_else(Error::denied)?;
    editor.client = client;
    editor.detached = false;
    editor.intent.as_mut().ok_or_else(Error::denied)?.generation = next;
    state_value(id, editor)
}

pub(super) fn discard(
    state: &mut State,
    client: Uuid,
    id: Uuid,
    requested_generation: u32,
    revision: u32,
) -> Result<EditingIntentValue> {
    if state.retired_editor_intents.get(&id) == Some(&client) {
        return Ok(EditingIntentValue::Retired {});
    }
    let editor = state.editors.get(&id).ok_or_else(Error::denied)?;
    if editor.client != client && !editor.detached {
        return Err(Error::denied());
    }
    generation(editor, requested_generation)?;
    if editor.revision != revision {
        return Err(conflict());
    }
    if editor.client != client && !can_admit(state, client) {
        return Err(Error::busy());
    }
    state.editors.remove(&id);
    state.retired_editor_intents.insert(id, client);
    Ok(EditingIntentValue::Retired {})
}
