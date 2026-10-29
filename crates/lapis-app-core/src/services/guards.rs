use super::{Error, Result, state::State};
use cap_std::fs::MetadataExt;
use uuid::Uuid;

pub(super) fn write(
    state: &State,
    path: &str,
    writer: Option<Uuid>,
    descendants: bool,
) -> Result<()> {
    state.root.validate_path(path, false)?;
    let target = state.root.entry_metadata(path).ok();
    for editor in state.editors.values() {
        if !editor.blocked() || (!editor.detached && Some(editor.client) == writer) {
            continue;
        }
        let lexical = editor.path == path
            || (descendants
                && editor
                    .path
                    .strip_prefix(path)
                    .is_some_and(|tail| tail.starts_with('/')));
        let mut candidate = editor.path.as_str();
        let mut alias = false;
        loop {
            if target.as_ref().is_some_and(|target| {
                state.root.entry_metadata(candidate).is_ok_and(|source| {
                    source.dev() == target.dev() && source.ino() == target.ino()
                })
            }) {
                alias = true;
                break;
            }
            if !descendants {
                break;
            }
            let Some((parent, _)) = candidate.rsplit_once('/') else {
                break;
            };
            candidate = parent;
        }
        if lexical || alias {
            return Err(Error::busy());
        }
    }
    Ok(())
}
