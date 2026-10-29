use super::TaskWriteResult;
use crate::services::{Error, Work, operations};
use lapis_app_protocol::{ExpectedVersion, TextSnapshot, VaultPhase};
use lapis_task_services::TaskTransformError;

pub(super) fn apply(
    work: &Work,
    path: &str,
    expected: &ExpectedVersion,
    transform: impl FnOnce(&str) -> Result<String, TaskTransformError>,
) -> TaskWriteResult<TextSnapshot> {
    apply_with(work, path, expected, |raw| Ok((transform(raw)?, ()))).map(|(snapshot, ())| snapshot)
}

pub(super) fn apply_with<T>(
    work: &Work,
    path: &str,
    expected: &ExpectedVersion,
    transform: impl FnOnce(&str) -> Result<(String, T), TaskTransformError>,
) -> TaskWriteResult<(TextSnapshot, T)> {
    let original = {
        let state = work.vault.lock()?;
        if state.phase == VaultPhase::Closed {
            return Err(Error::closed().into());
        }
        operations::guard_existing_write(&state, path, expected, None)?
    };
    let (text, output) = transform(&original.text)?;
    let state = work.vault.lock()?;
    if state.phase == VaultPhase::Closed {
        return Err(Error::closed().into());
    }
    let version = operations::write_if_changed(&state, path, &text, expected, None)?;
    Ok((
        TextSnapshot {
            path: path.into(),
            text,
            version,
        },
        output,
    ))
}
