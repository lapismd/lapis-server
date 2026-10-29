use super::super::{Error, Result};
use crate::{record_store::StoredRecord, turns::queue_capture};
use lapis_app_protocol::{ErrorCode, turns::TurnTarget};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, io};
use uuid::Uuid;

/// Native source admission only; retained capture fields are never launch authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueInput {
    pub acceptance_id: Uuid,
    pub conversation_id: Uuid,
    pub after_request_id: Uuid,
    pub scope_dir: String,
    pub target_id: String,
    pub capture: Value,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueuePhase {
    Pending,
    Started,
    Completed,
    Cancelled,
    Rejected,
    Unknown,
}
impl QueuePhase {
    pub(super) fn outcome(self) -> Option<&'static str> {
        match self {
            Self::Pending | Self::Started => None,
            Self::Completed => Some("completed"),
            Self::Cancelled => Some("cancelled"),
            Self::Rejected => Some("rejected"),
            Self::Unknown => Some("unknown"),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Target {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_selection: Option<lapis_app_protocol::queues::QueueSelection>,
    pub id: String,
    pub label: String,
    pub default_mode: String,
}
impl From<TurnTarget> for Target {
    fn from(value: TurnTarget) -> Self {
        Self {
            queue_selection: value.queue_selection,
            id: value.id,
            label: value.label,
            default_mode: value.default_mode,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueEntry {
    pub input: QueueInput,
    pub request_id: Uuid,
    pub phase: QueuePhase,
    pub receipt: Option<Value>,
    pub receipt_published: bool,
    pub(super) target: Target,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnQueue {
    pub schema_version: u32,
    pub vault_id: Uuid,
    pub conversation_id: Uuid,
    pub scope_dir: String,
    pub head_request_id: Uuid,
    pub paused: bool,
    pub barrier: Option<String>,
    pub entries: Vec<QueueEntry>,
}
pub(super) fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}
pub(super) fn profile(input: &QueueInput) -> Result<()> {
    queue_capture::validate_message(input.capture.clone()).map_err(|e| invalid(&e.message))?;
    let capture = &input.capture;
    let selection = &capture["selection"];
    let targets = selection["targets"].as_array().expect("validated targets");
    if input.acceptance_id.get_version_num() != 4
        || input.conversation_id.get_version_num() != 4
        || input.after_request_id.get_version_num() != 4
        || capture["state"] != "queued"
        || capture["id"].as_str().is_none_or(str::is_empty)
        || capture["user"]["id"].as_str().is_none_or(str::is_empty)
        || selection["runtime"] != "acp"
        || targets.len() != 1
        || !selection["attachments"]
            .as_array()
            .expect("validated attachments")
            .is_empty()
        || capture["user"].get("contextBlocks").is_some()
        || targets[0].get("agentProfile").is_some()
    {
        return Err(Error::new(
            ErrorCode::Unsupported,
            "Native queue supports one ACP plain-text recipient",
        ));
    }
    // Use a real native input validator, never compatibility configuration fallbacks.
    super::admission::input(input, Uuid::new_v4(), "ask")?;
    Ok(())
}
pub(super) fn receipt(entry: &QueueEntry) -> Result<Value> {
    let capture = queue_capture::validate_message(entry.input.capture.clone())
        .map_err(|e| invalid(&e.message))?;
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    queue_capture::create_receipt(
        &capture,
        entry
            .phase
            .outcome()
            .ok_or_else(|| invalid("Queue is unfinished"))?,
        &now,
    )
    .map(queue_capture::QueueReceipt::into_value)
    .map_err(|e| invalid(&e.message))
}
impl StoredRecord for TurnQueue {
    const KIND: &'static str = "queue";
    const MAX_RECORD_BYTES: usize = lapis_app_protocol::queues::QUEUE_RECORD_LIMIT;
    const MAX_RECORDS: usize = 256;
    fn id(&self) -> Uuid {
        self.conversation_id
    }
    fn validate(&self, vault: Uuid, id: Uuid) -> io::Result<()> {
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "Invalid native queue record");
        if self.schema_version != 1
            || self.vault_id != vault
            || self.conversation_id != id
            || id.get_version_num() != 4
            || self.head_request_id.get_version_num() != 4
            || self.entries.is_empty()
            || self.entries.len() > 128
            || self.barrier.as_ref().is_some_and(|s| s.len() > 256)
        {
            return Err(bad());
        }
        let mut ids = HashSet::new();
        let mut users = HashSet::new();
        let mut captures = HashSet::new();
        let mut pending = Vec::new();
        for entry in &self.entries {
            profile(&entry.input).map_err(|_| bad())?;
            if entry.input.conversation_id != id
                || entry.input.scope_dir != self.scope_dir
                || entry.request_id.get_version_num() != 4
                || entry.request_id == entry.input.acceptance_id
                || entry.target.id != entry.input.target_id
                || !ids.insert(entry.input.acceptance_id)
                || !ids.insert(entry.request_id)
                || !users.insert(entry.input.capture["user"]["id"].clone())
                || !captures.insert(entry.input.capture["id"].clone())
            {
                return Err(bad());
            }
            if matches!(entry.phase, QueuePhase::Pending | QueuePhase::Started) {
                pending.push(entry.input.capture.clone());
                if entry.receipt.is_some() || entry.receipt_published {
                    return Err(bad());
                }
            } else {
                let value = entry.receipt.as_ref().ok_or_else(bad)?;
                queue_capture::validate_receipt(value.clone()).map_err(|_| bad())?;
                if value["queued"] != entry.input.capture
                    || value["parentId"] != entry.input.capture["user"]["id"]
                    || value["outcome"] != entry.phase.outcome().expect("terminal")
                {
                    return Err(bad());
                }
            }
        }
        for entry in &self.entries {
            let id = entry.input.capture["id"].as_str().ok_or_else(bad)?;
            for outcome in ["completed", "cancelled", "rejected", "not_sent", "unknown"] {
                if users.contains(&Value::String(format!("queue:{id}:{outcome}"))) {
                    return Err(bad());
                }
            }
        }
        queue_capture::validate_queue(Some(Value::Array(pending))).map_err(|_| bad())?;
        Ok(())
    }
}
