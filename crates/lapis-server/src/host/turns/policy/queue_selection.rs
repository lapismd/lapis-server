use lapis_app_protocol::queues::QueueSelection;
use serde_json::json;

/// Validate declared capture data with Core's frozen codec, never launch authority.
pub(super) fn valid(selection: Option<&QueueSelection>) -> bool {
    let Some(selection) = selection else {
        return true;
    };
    let mut choices = json!({"targets":[selection.target],"runtime":"acp","attachments":[]});
    if let Some(thinking) = &selection.thinking {
        choices["thinking"] = json!(thinking);
    }
    lapis_app_core::turns::queue_capture::validate_message(json!({
        "version":1,"id":"operator-selection-validation","state":"queued",
        "user":{
            "schemaVersion":3,"id":"operator-user-validation","type":"message","role":"user",
            "text":"Validate declared selection", "recipients":[selection.recipient],
            "createdAt":"1970-01-01T00:00:00.000Z"
        },
        "selection":choices
    }))
    .is_ok()
}
