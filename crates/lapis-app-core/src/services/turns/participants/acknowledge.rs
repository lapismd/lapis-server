use super::super::{Data, Error, Result, Work, history::History, storage::TurnStorage};
use super::{invalid, store};
use crate::turns::participant_capture;
use lapis_app_protocol::ErrorCode;
use serde_json::Value;

/// An explicit next message may settle retained outcomes; observing never does.
pub(in crate::services::turns) fn history(
    data: &mut Data,
    storage: &TurnStorage,
    work: &Work,
    history: &mut Option<History>,
) -> Result<()> {
    let Some(history) = history else {
        return Ok(());
    };
    let Some(dispatch) = history.metadata.get("pendingDispatch").cloned() else {
        return Ok(());
    };
    participant_capture::validate_dispatch(dispatch.clone()).map_err(|e| invalid(&e.message))?;
    store::open(data, storage, work)?;
    let store = &data.participants.as_ref().expect("opened").store;
    let mut matched = None;
    for id in store.ids()? {
        let (message, _) = store.read(id)?.ok_or_else(super::super::failure)?;
        if message.input.dispatch["id"] == dispatch["id"]
            && history.metadata["id"] == message.conversation_id.to_string()
        {
            if matched.is_some() {
                return Err(invalid("Participant parent ownership is ambiguous"));
            }
            matched = Some(message);
        }
    }
    let message = matched.ok_or_else(|| {
        Error::new(
            ErrorCode::Conflict,
            "Unowned participant recovery requires its original owner",
        )
    })?;
    if message.deliveries.iter().any(|d| !d.state.terminal()) {
        return Err(Error::busy());
    }
    if dispatch["userEntryId"] != message.input.user["id"]
        || dispatch["deliveries"].as_array().map(Vec::len) != Some(message.deliveries.len())
    {
        return Err(invalid("Participant parent capture changed"));
    }
    let entries = history
        .transcript
        .text
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| invalid("Participant history changed"))?;
    if !entries.contains(&message.input.user) {
        return Err(invalid(
            "The exact retained participant parent is unavailable",
        ));
    }
    let time = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    for (index, delivery) in message.deliveries.iter().enumerate() {
        let mut capture = message.input.dispatch["deliveries"][index].clone();
        if dispatch["deliveries"][index]["id"] != capture["id"]
            || dispatch["deliveries"][index]["recipient"] != capture["recipient"]
        {
            return Err(invalid("Participant delivery capture changed"));
        }
        capture["state"] =
            serde_json::to_value(delivery.state).map_err(|_| super::super::failure())?;
        if dispatch["deliveries"][index]
            .get("agentBindingId")
            .is_some()
        {
            if dispatch["deliveries"][index]["agentBindingId"]
                != delivery.agent_binding_id.to_string()
            {
                return Err(invalid("Participant binding association changed"));
            }
            capture["agentBindingId"] = delivery.agent_binding_id.to_string().into();
        }
        let terminal =
            participant_capture::delivery_entry(&message.input.dispatch, &capture, &time)
                .map_err(|e| invalid(&e.message))?;
        if let Some(existing) = entries.iter().find(|v| v["id"] == terminal["id"]) {
            if existing["type"] != "agent.delivery"
                || existing["dispatchId"] != terminal["dispatchId"]
                || existing["parentId"] != terminal["parentId"]
                || existing["delivery"] != terminal["delivery"]
            {
                return Err(invalid("Immutable participant outcome changed"));
            }
        } else {
            history.transcript.text.push_str(&format!("{terminal}\n"));
            history.acknowledged = true;
        }
    }
    history
        .metadata
        .as_object_mut()
        .expect("validated metadata")
        .remove("pendingDispatch");
    Ok(())
}
