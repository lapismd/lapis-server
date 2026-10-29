use super::{Result, comments, fields::*, selection};
use serde_json::Value;

pub(crate) fn validate(user: &Value) -> Result<Vec<String>> {
    record(user, "Queued authored message")?;
    fields(
        user,
        &[
            "schemaVersion",
            "id",
            "type",
            "role",
            "text",
            "recipients",
            "createdAt",
            "contextBlocks",
            "provenance",
        ],
    )?;
    if user["schemaVersion"] != 3
        || user["type"] != "message"
        || user["role"] != "user"
        || user["text"].as_str().is_none_or(|s| s.len() > 32768)
    {
        return fail("Invalid queued authored message");
    }
    string(user, "id", "Queued authored message")?;
    timestamp(user, "createdAt", "Queued authored message")?;
    if let Some(provenance) = user.get("provenance") {
        record(provenance, "Queued provenance")?;
        fields(provenance, &["originClass", "sourceKind"])?;
        if provenance["originClass"] != "owner" || provenance["sourceKind"] != "user-message" {
            return fail("Invalid queued message provenance");
        }
    }
    let ids = selection::recipients(user.get("recipients"))?;
    let comments = comments::comments(user.get("contextBlocks"))?;
    if ids.is_empty()
        || (crate::text::trim(user["text"].as_str().expect("validated text")).is_empty()
            && comments == 0)
    {
        return fail("Queued message requires recipients and authored content");
    }
    Ok(ids)
}
