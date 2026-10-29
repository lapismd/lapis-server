use super::{
    TurnJournal,
    types::{invalid, now},
};
use lapis_app_protocol::turns::{TurnInput, TurnPhase};
use std::io;
use uuid::Uuid;

pub(super) fn select(journal: &TurnJournal, input: &TurnInput) -> io::Result<(Uuid, u64)> {
    let timestamp = now()?;
    let Some(cursor) = &input.continuation else {
        return Ok((Uuid::new_v4(), timestamp));
    };
    let parent = journal
        .read(cursor.after_request_id)?
        .ok_or_else(|| invalid("Native conversation predecessor is unavailable"))?;
    if parent.conversation_id != cursor.conversation_id || parent.input.scope_dir != input.scope_dir
    {
        return Err(invalid(
            "Native conversation predecessor does not match its scope",
        ));
    }
    if matches!(parent.phase, TurnPhase::Accepted | TurnPhase::Running) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "Native conversation predecessor is unfinished",
        ));
    }
    if journal.list()?.iter().any(|record| {
        record.conversation_id == parent.conversation_id
            && record.request_id != parent.input.request_id
            && record.accepted_at_ms >= parent.accepted_at_ms
    }) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "Native conversation predecessor is stale",
        ));
    }
    let timestamp = timestamp.max(
        parent
            .updated_at_ms
            .checked_add(1)
            .ok_or_else(|| invalid("Native conversation timestamp exceeds its limit"))?,
    );
    Ok((parent.conversation_id, timestamp))
}
