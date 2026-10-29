use super::super::{Owner, Result, Work};
use super::{QueueInput, Target, invalid};
use lapis_app_protocol::turns::{TurnContinuation, TurnInput, TurnOptions};
use uuid::Uuid;

pub(super) fn input(input: &QueueInput, request_id: Uuid, default_mode: &str) -> Result<TurnInput> {
    let target = &input.capture["selection"]["targets"][0];
    let thinking = target
        .get("thinking")
        .filter(|value| !value.is_null())
        .or_else(|| input.capture["selection"].get("thinking"));
    crate::turns::normalize(TurnInput {
        request_id,
        target_id: input.target_id.clone(),
        scope_dir: input.scope_dir.clone(),
        prompt: input.capture["user"]["text"]
            .as_str()
            .unwrap_or_default()
            .into(),
        context: None,
        continuation: Some(TurnContinuation {
            conversation_id: input.conversation_id,
            after_request_id: input.after_request_id,
        }),
        options: TurnOptions {
            mode: target["executionMode"]
                .as_str()
                .unwrap_or(default_mode)
                .into(),
            model: target["model"]["model"].as_str().map(String::from),
            effort: thinking
                .and_then(|v| v.as_str())
                .filter(|s| !matches!(*s, "default" | ""))
                .map(String::from),
            ..Default::default()
        },
    })
    .map_err(Into::into)
}
pub(super) fn target(owner: &Owner, work: &Work, input: &QueueInput) -> Result<Target> {
    let scope = {
        let state = work.vault.lock()?;
        if input.scope_dir.is_empty() {
            state.root.try_clone()?
        } else {
            state.root.open_child(&input.scope_dir)?
        }
    };
    scope.verify_physical_root()?;
    let matches = owner
        .storage
        .policy
        .targets(work.vault.id, &scope)
        .map_err(|_| invalid("Native queue target catalogue is unavailable"))?
        .into_iter()
        .filter(|t| t.id == input.target_id)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(invalid("Native queue target is unavailable or ambiguous"));
    }
    Ok(matches.into_iter().next().expect("one target").into())
}
pub(super) fn head(
    data: &super::super::Data,
    input: &QueueInput,
) -> Result<lapis_app_protocol::turns::TurnRecord> {
    let record = data
        .journal
        .read(input.after_request_id)?
        .ok_or_else(|| invalid("Native queue predecessor is unavailable"))?;
    if record.conversation_id != input.conversation_id
        || record.input.scope_dir != input.scope_dir
        || data.journal.list()?.iter().any(|r| {
            r.conversation_id == record.conversation_id
                && r.request_id != record.input.request_id
                && r.accepted_at_ms >= record.accepted_at_ms
        })
    {
        return Err(invalid(
            "Native queue predecessor is stale or has another scope",
        ));
    }
    Ok(record)
}

/// Portable authorship and generated receipt IDs are independent namespaces.
pub(super) fn authored(
    input: &QueueInput,
    selected: &super::super::launch::Selected,
) -> Result<()> {
    let history = selected
        .history
        .as_ref()
        .ok_or_else(|| invalid("Native queue history is unavailable"))?;
    for line in history.transcript.text.lines() {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|_| invalid("Native queue history changed"))?;
        if value["id"] == input.capture["user"]["id"] {
            return Err(invalid(
                "Queued authored identity already exists in the conversation",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
