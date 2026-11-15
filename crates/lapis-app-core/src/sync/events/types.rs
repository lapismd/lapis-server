use super::super::{invalid, validate_id, validate_object_id};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, io};

pub(super) const MAX_EVENT_PAYLOAD_BYTES: usize = 512 * 1024;
pub(super) const MAX_EVENT_RECORD_BYTES: usize = 2 * 1024 * 1024;
pub(super) const MAX_EVENT_DEPENDENCIES: usize = 4096;
pub(super) const MAX_HEAD_REVISIONS: usize = 64;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventKind {
    Revision,
    Head,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    pub id: String,
    pub kind: EventKind,
    pub device_id: Option<String>,
    pub sealed: Vec<u8>,
    pub object_dependencies: Vec<String>,
    pub event_dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedEvent {
    pub event: StoredEvent,
    pub reused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedHead {
    pub event_reused: bool,
    pub pointer_reused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadSnapshot {
    pub device_id: String,
    pub revision_ids: Vec<String>,
    pub event: StoredEvent,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct EventRecord {
    pub id: String,
    pub vault: String,
    pub destination: String,
    pub kind: EventKind,
    pub device_id: Option<String>,
    pub payload_sha256: String,
    pub sealed_payload: String,
    pub object_dependencies: Vec<String>,
    pub event_dependencies: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct HeadPointer {
    pub vault: String,
    pub destination: String,
    pub device_id: String,
    pub event_id: String,
}

impl EventRecord {
    pub(super) fn validate(&self, vault: &str, destination: &str, id: &str) -> io::Result<()> {
        if self.id != id
            || self.vault != vault
            || self.destination != destination
            || self.payload_sha256.len() != 64
            || !self
                .payload_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid("Invalid durable Sync event"));
        }
        validate_object_id(&self.id)?;
        validate_ids(&self.object_dependencies, false, MAX_EVENT_DEPENDENCIES)?;
        validate_ids(&self.event_dependencies, false, MAX_EVENT_DEPENDENCIES)?;
        match (&self.kind, &self.device_id) {
            (EventKind::Revision, None) => {}
            (EventKind::Head, Some(device)) => {
                validate_id(device)?;
                if self.event_dependencies.is_empty()
                    || self.event_dependencies.len() > MAX_HEAD_REVISIONS
                {
                    return Err(invalid("Invalid durable Sync HEAD dependencies"));
                }
            }
            _ => return Err(invalid("Invalid durable Sync event authority")),
        }
        Ok(())
    }

    pub(super) fn open(&self) -> io::Result<StoredEvent> {
        let sealed = STANDARD_NO_PAD
            .decode(&self.sealed_payload)
            .map_err(|_| invalid("Durable Sync event payload is unreadable"))?;
        if sealed.is_empty()
            || sealed.len() > MAX_EVENT_PAYLOAD_BYTES
            || format!("{:x}", Sha256::digest(&sealed)) != self.payload_sha256
        {
            return Err(invalid(
                "Durable Sync event payload failed integrity validation",
            ));
        }
        Ok(StoredEvent {
            id: self.id.clone(),
            kind: self.kind,
            device_id: self.device_id.clone(),
            sealed,
            object_dependencies: self.object_dependencies.clone(),
            event_dependencies: self.event_dependencies.clone(),
        })
    }
}

impl HeadPointer {
    pub(super) fn validate(&self, vault: &str, destination: &str, device: &str) -> io::Result<()> {
        if self.vault != vault || self.destination != destination || self.device_id != device {
            return Err(invalid("Invalid durable Sync HEAD pointer"));
        }
        validate_id(&self.device_id)?;
        validate_object_id(&self.event_id)
    }
}

pub(super) fn validate_ids(values: &[String], require_one: bool, max: usize) -> io::Result<()> {
    if values.len() > max || (require_one && values.is_empty()) {
        return Err(invalid("Invalid durable Sync dependency count"));
    }
    let mut seen = HashSet::with_capacity(values.len());
    for value in values {
        validate_object_id(value)?;
        if !seen.insert(value) {
            return Err(invalid("Duplicate durable Sync dependency"));
        }
    }
    Ok(())
}
