use super::{CaptureError, Result, fields::*};
use serde_json::Value;
use std::collections::HashSet;

fn identity<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    let text = string(value, key, "Comment capture")?;
    if utf16(text) > 512 || text.chars().any(|c| c <= '\u{1f}') {
        return fail("Invalid comment capture identity");
    }
    Ok(text)
}
fn range(value: &Value) -> Result<(u64, u64)> {
    let (Some(from), Some(to)) = (safe_integer(&value["from"]), safe_integer(&value["to"])) else {
        return fail("Invalid comment capture range");
    };
    if to < from {
        return fail("Invalid comment capture range");
    }
    Ok((from, to))
}
pub(super) fn comments(value: Option<&Value>) -> Result<usize> {
    let Some(value) = value else { return Ok(0) };
    let Some(blocks) = value.as_array().filter(|v| v.len() <= 16) else {
        return fail("Invalid comment context list");
    };
    if bytes(value) > 65536 {
        return fail("Comment context exceeds 64 KiB");
    }
    let mut ids = HashSet::new();
    for block in blocks {
        record(block, "Comment context")?;
        let id = identity(block, "id")?;
        record(&block["metadata"], "Comment context metadata")?;
        if block["kind"] != "transcript-comments"
            || block["metadata"]["submissionId"] != id
            || !ids.insert(id)
        {
            return fail("Invalid comment context identity");
        }
        let raw = string(block, "content", "Comment context")?;
        let content: Value = serde_json::from_str(raw).map_err(|error| CaptureError {
            name: "SyntaxError".into(),
            message: json_error(raw, &error),
        })?;
        record(&content, "Comment capture")?;
        let Some(comments) = content["comments"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 64)
        else {
            return fail("Invalid captured comment list");
        };
        let mut comment_ids = HashSet::new();
        for comment in comments {
            record(comment, "Captured comment")?;
            identity(comment, "threadId")?;
            if !comment_ids.insert(identity(comment, "commentId")?) {
                return fail("Duplicate captured comment");
            }
            string(comment, "text", "Captured comment")?;
            let author = &comment["author"];
            record(author, "Comment author")?;
            identity(author, "id")?;
            identity(author, "name")?;
            let anchor = &comment["anchor"];
            record(anchor, "Comment anchor")?;
            identity(anchor, "blockId")?;
            identity(anchor, "revision")?;
            let (from, to) = range(anchor)?;
            if anchor["quote"]
                .as_str()
                .is_none_or(|s| utf16(s) as u64 != to - from)
            {
                return fail("Comment quote does not match its captured range");
            }
            if let Some(table) = anchor.get("tableContext") {
                record(table, "Comment table context")?;
                let (_, to) = range(table)?;
                if table["markdown"]
                    .as_str()
                    .is_none_or(|s| to > utf16(s) as u64)
                {
                    return fail("Invalid captured table context");
                }
            }
        }
    }
    Ok(blocks.len())
}
fn json_error(raw: &str, error: &serde_json::Error) -> String {
    // V8 exposes parser text, not a product validation error. Retain the
    // reference's invalid-token diagnostic while preserving the SyntaxError kind.
    let token = raw.chars().nth(error.column().saturating_sub(1));
    if raw.len() <= 20
        && let Some(token) = token
    {
        return format!("Unexpected token '{token}', {raw:?} is not valid JSON");
    }
    if error.is_eof() {
        return "Unexpected end of JSON input".into();
    }
    format!("Invalid captured JSON: {error}")
}
