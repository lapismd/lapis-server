//! Retained data compatibility only. This module creates no dispatch authority.
mod comments;
pub(crate) mod fields;
mod receipt;
pub(crate) mod selection;
mod user;
use fields::*;
use serde::Serialize;
use serde_json::{Value, json};
use std::{collections::HashSet, fmt};

pub type Result<T> = std::result::Result<T, CaptureError>;
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CaptureError {
    pub name: String,
    pub message: String,
}
impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for CaptureError {}

/// Validated retained data, including optional-field omission and original paths.
/// It is not a native launch, file capability, queue claim or execution binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct QueueCapture(Value);
impl QueueCapture {
    pub fn as_value(&self) -> &Value {
        &self.0
    }
    pub fn into_value(self) -> Value {
        self.0
    }
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(&self.0).expect("retained JSON")
    }
}
pub fn validate_message(value: Value) -> Result<QueueCapture> {
    record(&value, "Queued message")?;
    fields(
        &value,
        &["version", "id", "state", "dispatchId", "user", "selection"],
    )?;
    if value["version"] != 1 || !one_of(&value["state"], &["queued", "dispatching"]) {
        return fail("Invalid queued message identity or state");
    }
    if utf16(string(&value, "id", "Queued message")?) > 512 {
        return fail("Invalid queued message identity or state");
    }
    if value["state"] == "dispatching" {
        string(&value, "dispatchId", "Queued message")?;
    } else if value.get("dispatchId").is_some() {
        return fail("Unstarted queue item cannot retain a dispatch");
    }
    let ids = user::validate(&value["user"])?;
    selection::selection(&value["selection"], &ids)?;
    Ok(QueueCapture(value))
}
/// Omitted metadata is an empty queue; explicit null remains invalid.
pub fn validate_queue(value: Option<Value>) -> Result<Vec<QueueCapture>> {
    let value = value.unwrap_or_else(|| json!([]));
    let Some(items) = value.as_array().filter(|a| a.len() <= 16) else {
        return fail("Pending message queue limit exceeded");
    };
    if bytes(&value) > 128 * 1024 {
        return fail("Pending message queue limit exceeded");
    }
    let captures = items
        .iter()
        .cloned()
        .map(validate_message)
        .collect::<Result<Vec<_>>>()?;
    let mut ids = HashSet::new();
    let mut users = HashSet::new();
    if captures.iter().any(|c| {
        !ids.insert(c.0["id"].as_str().expect("validated id"))
            || !users.insert(c.0["user"]["id"].as_str().expect("validated authored id"))
    }) {
        return fail("Queued message identities must be distinct");
    }
    Ok(captures)
}
/// Runtime configuration data only. Explicit provider-default thinking is saved
/// in the capture and intentionally omitted from the requested effort field.
pub fn configuration_frame(input: &Value, provider: &str) -> Result<Value> {
    selection::mode(input.get("executionMode"))?;
    let default = match provider {
        "codex" => "read-only",
        "opencode" => "plan",
        _ => "ask",
    };
    let mut result = json!({"type":"configure","mode":input.get("executionMode").filter(|v| !v.is_null()).cloned().unwrap_or_else(|| Value::from(default))});
    if let Some(model) = input.get("model").filter(|v| !v.is_null()) {
        result["model"] = model["model"].clone();
    }
    if let Some(thinking) = input
        .get("thinking")
        .filter(|v| !v.is_null() && *v != "default" && *v != "")
    {
        result["effort"] = thinking.clone();
    }
    Ok(result)
}
pub use receipt::{QueueReceipt, create_receipt, validate_receipt};

pub(crate) fn validate_user(user: &Value) -> Result<Vec<String>> {
    user::validate(user)
}

/// Validate installation-declared captured choices with the retained source codec.
/// This projection grants no launch or provider authority.
pub fn validate_declared_selection(
    declared: &lapis_app_protocol::queues::QueueSelection,
) -> Result<()> {
    let ids = selection::recipients(Some(&json!([declared.recipient])))?;
    let mut choices = json!({"targets":[declared.target],"runtime":"acp","attachments":[]});
    if let Some(thinking) = &declared.thinking {
        choices["thinking"] = json!(thinking);
    }
    selection::selection(&choices, &ids)
}
