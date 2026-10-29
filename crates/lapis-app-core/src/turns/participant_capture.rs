//! Retained addressed-message data only; tags create no execution authority.
use super::queue_capture::{Result, fields::*, selection};
use serde_json::{Value, json};
use std::collections::HashSet;

pub const STATES: [&str; 7] = [
    "pending",
    "started",
    "completed",
    "rejected",
    "unknown",
    "not_sent",
    "cancelled",
];
pub fn validate_delivery(value: Value) -> Result<Value> {
    let data = record(&value, "Agent delivery")?;
    if data
        .keys()
        .any(|key| !["id", "recipient", "state", "agentBindingId"].contains(&key.as_str()))
    {
        return fail("Agent delivery contains unsupported fields");
    }
    if utf16(string(&value, "id", "Agent delivery")?) > 512
        || !value["state"].as_str().is_some_and(|s| STATES.contains(&s))
    {
        return fail("Agent delivery identity or state is invalid");
    }
    selection::recipients(Some(&json!([value["recipient"]])))?;
    if value
        .get("agentBindingId")
        .is_some_and(|v| v.as_str().is_none_or(|s| s.is_empty() || utf16(s) > 512))
    {
        return fail("Agent delivery binding is invalid");
    }
    if value["state"] == "started" && value.get("agentBindingId").is_none() {
        return fail("Started delivery requires a binding");
    }
    Ok(value)
}
pub fn validate_dispatch(value: Value) -> Result<Value> {
    let data = record(&value, "Pending addressed message")?;
    if value["version"] != 1
        || data.keys().any(|key| {
            !["version", "id", "userEntryId", "startedAt", "deliveries"].contains(&key.as_str())
        })
    {
        return fail("Pending addressed message format is unsupported");
    }
    for key in ["id", "userEntryId"] {
        if utf16(string(&value, key, "Pending addressed message")?) > 512 {
            return fail("Pending addressed message identity is too long");
        }
    }
    timestamp(&value, "startedAt", "Pending addressed message")?;
    let Some(deliveries) = value["deliveries"]
        .as_array()
        .filter(|v| (1..=4).contains(&v.len()))
    else {
        return fail("An addressed message requires one to four deliveries");
    };
    for delivery in deliveries {
        validate_delivery(delivery.clone())?;
    }
    selection::recipients(Some(&Value::Array(
        deliveries.iter().map(|d| d["recipient"].clone()).collect(),
    )))?;
    let ids: HashSet<_> = deliveries
        .iter()
        .map(|d| d["id"].as_str().expect("validated identity"))
        .collect();
    if ids.len() != deliveries.len() {
        return fail("Agent delivery identities must be distinct");
    }
    Ok(value)
}
pub fn delivery_entry(dispatch: &Value, delivery: &Value, created_at: &str) -> Result<Value> {
    validate_dispatch(dispatch.clone())?;
    validate_delivery(delivery.clone())?;
    Ok(json!({"schemaVersion":3,"type":"agent.delivery",
        "id":format!("delivery:{}:{}",delivery["id"].as_str().expect("validated id"),delivery["state"].as_str().expect("validated state")),
        "parentId":dispatch["userEntryId"],"dispatchId":dispatch["id"],"createdAt":created_at,
        "delivery":delivery,"provenance":{"originClass":"system","sourceKind":"app-system"}}))
}
