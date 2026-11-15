use super::super::invalid;
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io;
use uuid::Uuid;

pub(super) const MAX_SEALED_BYTES: usize = 512 * 1024;
pub(super) const MAX_RECORD_BYTES: usize = 768 * 1024;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationKind {
    Outbox,
    Apply,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationPhase {
    Staged,
    Accepted,
    Applied,
    Unknown,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationRecord {
    pub id: Uuid,
    pub vault: Uuid,
    pub destination: String,
    pub request_id: String,
    pub kind: OperationKind,
    pub phase: OperationPhase,
    pub ciphertext_sha256: String,
    pub sealed_payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageOutcome {
    pub record: OperationRecord,
    pub reused: bool,
}

impl OperationRecord {
    pub(super) fn validate(&self, vault: Uuid, destination: &str, id: Uuid) -> io::Result<()> {
        if self.id != id
            || self.id.get_version_num() != 4
            || self.vault != vault
            || self.destination != destination
            || self.ciphertext_sha256.len() != 64
            || !self
                .ciphertext_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid("Invalid durable Sync operation"));
        }
        validate_request_id(&self.request_id)
    }
}

pub(super) fn decode_sealed(record: &OperationRecord) -> io::Result<Vec<u8>> {
    let sealed = STANDARD_NO_PAD
        .decode(&record.sealed_payload)
        .map_err(|_| invalid("Durable Sync payload is unreadable"))?;
    if sealed.is_empty()
        || sealed.len() > MAX_SEALED_BYTES
        || format!("{:x}", Sha256::digest(&sealed)) != record.ciphertext_sha256
    {
        return Err(invalid("Durable Sync payload failed integrity validation"));
    }
    Ok(sealed)
}

pub(super) fn validate_request_id(value: &str) -> io::Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(invalid("Invalid durable Sync request identity"));
    }
    Ok(())
}

pub(super) fn allowed_transition(from: OperationPhase, to: OperationPhase) -> bool {
    matches!(
        (from, to),
        (OperationPhase::Staged, OperationPhase::Accepted)
            | (OperationPhase::Staged, OperationPhase::Unknown)
            | (OperationPhase::Accepted, OperationPhase::Applied)
            | (OperationPhase::Accepted, OperationPhase::Unknown)
            | (OperationPhase::Unknown, OperationPhase::Accepted)
    )
}
