use super::{QueueCapture, Result, fields::*, validate_message};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct QueueReceipt(Value);
impl QueueReceipt {
    pub fn as_value(&self) -> &Value {
        &self.0
    }
    pub fn into_value(self) -> Value {
        self.0
    }
}
/// Decode historical attribution exactly; parent and receipt IDs are not native
/// admission evidence. The retained validator does not assert their relationship.
pub fn validate_receipt(value: Value) -> Result<QueueReceipt> {
    record(&value, "Transcript entry")?;
    if ![1, 2, 3].iter().any(|v| value["schemaVersion"] == *v) {
        return fail("Transcript entry uses an unsupported required schema version");
    }
    string(&value, "id", "Transcript entry")?;
    timestamp(&value, "createdAt", "Transcript entry")?;
    if value["type"] != "queue.outcome" {
        return fail("Transcript entry type is unsupported");
    }
    if let Some(source) = value.get("source").filter(|v| !v.is_null()) {
        record(source, "Transcript entry source")?;
        string(source, "sessionId", "Transcript entry source")?;
        string(source, "runId", "Transcript entry source")?;
        if safe_integer(&source["sequence"]).is_none() {
            return fail("Transcript entry source.sequence is invalid");
        }
        if source
            .get("frameIndex")
            .is_some_and(|v| safe_integer(v).is_none())
        {
            return fail("Transcript entry source.frameIndex is invalid");
        }
    }
    if let Some(p) = value.get("provenance").filter(|v| !v.is_null()) {
        record(p, "Transcript entry provenance")?;
        if !one_of(
            &p["originClass"],
            &["owner", "agent", "untrusted", "system"],
        ) {
            return fail("Transcript entry provenance.originClass is invalid");
        }
        if !one_of(
            &p["sourceKind"],
            &[
                "user-message",
                "assistant-message",
                "runtime-output",
                "owner-response",
                "app-system",
            ],
        ) {
            return fail("Transcript entry provenance.sourceKind is invalid");
        }
    }
    validate_message(value["queued"].clone())?;
    if !one_of(
        &value["outcome"],
        &["completed", "cancelled", "rejected", "not_sent", "unknown"],
    ) {
        return fail("Invalid queue outcome");
    }
    Ok(QueueReceipt(value))
}
/// Construct a correctly attributed receipt without creating a dispatch claim.
/// The caller owns timestamp selection; this pure codec neither schedules nor writes.
pub fn create_receipt(
    capture: &QueueCapture,
    outcome: &str,
    created_at: &str,
) -> Result<QueueReceipt> {
    validate_receipt(
        json!({"schemaVersion":3,"type":"queue.outcome","id":format!("queue:{}:{outcome}",capture.as_value()["id"].as_str().expect("validated identity")),"parentId":capture.as_value()["user"]["id"],"createdAt":created_at,"queued":capture.as_value(),"outcome":outcome}),
    )
}
