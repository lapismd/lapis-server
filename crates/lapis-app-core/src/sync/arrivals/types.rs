use super::super::{invalid, validate_object_id};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io;
use uuid::Uuid;

pub(super) const MAX_MEMBERSHIP_BYTES: usize = 512 * 1024;
pub(super) const MAX_RECORD_BYTES: usize = 1024 * 1024;
pub(super) const MAX_PAGE_SIZE: usize = 256;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArrivalKind {
    Revision,
    Head,
    Membership,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArrivalCursor {
    pub generation: Uuid,
    pub snapshot: u64,
    pub after: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrivalItem {
    pub sequence: u64,
    pub kind: ArrivalKind,
    pub event_id: String,
    pub membership_generation: Option<u64>,
    pub predecessor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedArrival {
    pub item: ArrivalItem,
    pub reused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrivalPage {
    pub items: Vec<ArrivalItem>,
    pub cursor: ArrivalCursor,
    pub next_cursor: Option<ArrivalCursor>,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatchUpOutcome {
    Page(ArrivalPage),
    RestartRequired,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ArrivalRecord {
    pub sequence: u64,
    pub vault: String,
    pub destination: String,
    pub kind: ArrivalKind,
    pub event_id: String,
    pub membership_generation: Option<u64>,
    pub predecessor: Option<String>,
    pub payload_sha256: Option<String>,
    pub sealed_payload: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ArrivalState {
    pub vault: String,
    pub destination: String,
    pub cursor_generation: Uuid,
    pub last_sequence: u64,
    pub membership_generation: Option<u64>,
    pub membership_event_id: Option<String>,
}

impl ArrivalRecord {
    pub(super) fn validate(&self, vault: &str, destination: &str) -> io::Result<()> {
        if self.sequence == 0 || self.vault != vault || self.destination != destination {
            return Err(invalid("Invalid durable Sync arrival authority"));
        }
        validate_object_id(&self.event_id)?;
        match self.kind {
            ArrivalKind::Revision | ArrivalKind::Head => {
                if self.membership_generation.is_some()
                    || self.predecessor.is_some()
                    || self.payload_sha256.is_some()
                    || self.sealed_payload.is_some()
                {
                    return Err(invalid("Invalid durable Sync event arrival"));
                }
            }
            ArrivalKind::Membership => {
                let generation = self
                    .membership_generation
                    .ok_or_else(|| invalid("Missing Sync membership generation"))?;
                if (generation == 0) != self.predecessor.is_none() {
                    return Err(invalid("Invalid Sync membership predecessor"));
                }
                if let Some(predecessor) = &self.predecessor {
                    validate_object_id(predecessor)?;
                }
                self.open_membership()?;
            }
        }
        Ok(())
    }

    pub(super) fn item(&self) -> ArrivalItem {
        ArrivalItem {
            sequence: self.sequence,
            kind: self.kind,
            event_id: self.event_id.clone(),
            membership_generation: self.membership_generation,
            predecessor: self.predecessor.clone(),
        }
    }

    pub(super) fn open_membership(&self) -> io::Result<Vec<u8>> {
        let encoded = self
            .sealed_payload
            .as_ref()
            .ok_or_else(|| invalid("Missing durable Sync membership payload"))?;
        let bytes = STANDARD_NO_PAD
            .decode(encoded)
            .map_err(|_| invalid("Durable Sync membership payload is unreadable"))?;
        let expected = self
            .payload_sha256
            .as_deref()
            .ok_or_else(|| invalid("Missing durable Sync membership integrity"))?;
        if bytes.is_empty()
            || bytes.len() > MAX_MEMBERSHIP_BYTES
            || expected.len() != 64
            || format!("{:x}", Sha256::digest(&bytes)) != expected
        {
            return Err(invalid(
                "Durable Sync membership payload failed integrity validation",
            ));
        }
        Ok(bytes)
    }
}

impl ArrivalState {
    pub(super) fn validate(&self, vault: &str, destination: &str) -> io::Result<()> {
        if self.vault != vault
            || self.destination != destination
            || self.cursor_generation.is_nil()
            || self.membership_generation.is_some() != self.membership_event_id.is_some()
        {
            return Err(invalid("Invalid durable Sync arrival state"));
        }
        if let Some(id) = &self.membership_event_id {
            validate_object_id(id)?;
        }
        Ok(())
    }
}
