use lapis_app_protocol::turns::{TurnInput, TurnPhase, TurnRecord};
use std::io;
use uuid::Uuid;

pub const MAX_PROMPT_BYTES: usize = 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_RECORD_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_RECORDS: usize = 4096;

pub(crate) fn normalize(mut input: TurnInput) -> io::Result<TurnInput> {
    if input.request_id.get_version_num() != 4
        || input.target_id.trim().is_empty()
        || input.target_id.len() > 256
        || input.target_id.chars().any(char::is_control)
        || input.prompt.trim().is_empty()
        || input.prompt.len() > MAX_PROMPT_BYTES
        || input.continuation.as_ref().is_some_and(|cursor| {
            cursor.conversation_id.get_version_num() != 4
                || cursor.after_request_id.get_version_num() != 4
                || cursor.after_request_id == input.request_id
        })
        || input.options.max_duration_ms == 0
        || input.options.max_duration_ms > 86_400_000
        || input.options.mode.trim().is_empty()
        || [
            Some(&input.options.mode),
            input.options.model.as_ref(),
            input.options.effort.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
        || input
            .context
            .as_ref()
            .is_some_and(|s| s.len() > MAX_PROMPT_BYTES)
    {
        return Err(invalid("Invalid native turn input"));
    }
    let raw = input.scope_dir.trim().replace('\\', "/");
    input.scope_dir = if matches!(raw.as_str(), "" | "." | "/") {
        String::new()
    } else {
        let scope = raw.strip_prefix("./").unwrap_or(&raw).trim_end_matches('/');
        if scope.len() > 4096
            || scope.contains([':', '\0'])
            || scope
                .split('/')
                .any(|part| matches!(part, "" | "." | ".." | ".lapis"))
        {
            return Err(invalid("Invalid native conversation scope"));
        }
        scope.into()
    };
    Ok(input)
}
pub(super) fn validate(record: &TurnRecord, vault: Uuid, id: Uuid) -> io::Result<()> {
    if record.schema_version != 1
        || record.vault_id != vault
        || record.vault_id.is_nil()
        || record.input.request_id != id
        || record.conversation_id.get_version_num() != 4
        || record
            .input
            .continuation
            .as_ref()
            .is_some_and(|cursor| cursor.conversation_id != record.conversation_id)
        || normalize(record.input.clone())? != record.input
        || record.updated_at_ms < record.accepted_at_ms
        || record.updated_at_ms > 253_402_300_799_999
        || record
            .output
            .as_ref()
            .is_some_and(|s| s.len() > MAX_OUTPUT_BYTES)
        || record
            .failure_code
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
        || (record.phase == TurnPhase::Completed && record.output.is_none())
        || match record.phase {
            TurnPhase::Accepted => record.output.is_some() || record.failure_code.is_some(),
            TurnPhase::Running | TurnPhase::Completed | TurnPhase::Cancelled => {
                record.failure_code.is_some()
            }
            TurnPhase::Failed | TurnPhase::Interrupted | TurnPhase::Unknown => {
                record.failure_code.is_none()
            }
        }
    {
        return Err(invalid("Invalid native turn record"));
    }
    Ok(())
}
pub enum TurnOutcome {
    Completed(String),
    Failed(String),
    Cancelled,
    Unknown(String),
}

pub(super) fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
pub(super) fn now() -> io::Result<u64> {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| invalid("Native clock is unavailable"))?
            .as_millis(),
    )
    .map_err(|_| invalid("Native clock exceeds its bound"))
}

impl crate::record_store::StoredRecord for TurnRecord {
    const KIND: &'static str = "turn";
    const MAX_RECORD_BYTES: usize = MAX_RECORD_BYTES;
    const MAX_RECORDS: usize = MAX_RECORDS;
    fn id(&self) -> Uuid {
        self.input.request_id
    }
    fn validate(&self, vault: Uuid, id: Uuid) -> io::Result<()> {
        validate(self, vault, id)
    }
}
