use super::super::{Error, Result, Work, history::History, participants::ParticipantMessage};
use super::publication::Slot;
use super::{Conversation, Publication, expected, line, timestamp};
use crate::vault::sync_directory;
use lapis_app_protocol::{ErrorCode, turns::TurnRecord};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub(super) fn open(
    work: &Work,
    record: &TurnRecord,
    history: Option<History>,
    authored: Option<Value>,
    group: Option<&ParticipantMessage>,
) -> Result<Vec<Conversation>> {
    let parent = [record.input.scope_dir.as_str(), ".lapis/agents/sessions"]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    let directory = format!("{parent}/{}", record.conversation_id);
    if history.is_none() {
        let state = work.vault.lock()?;
        state.root.verify_physical_root()?;
        super::super::super::guards::write(&state, &parent, None, true)?;
        let mut containing = state.root.directory_handle()?;
        let mut path = String::new();
        for part in directory.split('/') {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(part);
            match state.root.mkdir(&path, false) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && path != directory => {}
                Err(e) => return Err(e.into()),
            }
            sync_directory(&containing)?;
            containing = state.root.open_directory(&path)?;
        }
        sync_directory(&containing)?;
    }
    let created = timestamp(record.accepted_at_ms);
    let (mut metadata, metadata_expected, agents, agents_expected, prior, transcript_expected) =
        match history {
            Some(mut history) => {
                for (path, version) in [
                    (
                        format!("{directory}/metadata.yaml"),
                        &history.metadata_version,
                    ),
                    (history.agents.path.clone(), &history.agents.version),
                    (history.transcript.path.clone(), &history.transcript.version),
                ] {
                    if work.read_text(&path)?.version != *version {
                        return Err(Error::new(
                            ErrorCode::Conflict,
                            "Native conversation changed before publication",
                        ));
                    }
                }
                if history.acknowledged {
                    history.transcript.version = work.write_text(
                        &history.transcript.path,
                        &history.transcript.text,
                        &lapis_app_protocol::ExpectedVersion::Version {
                            version: history.transcript.version.clone(),
                        },
                    )?;
                }
                (
                    history.metadata,
                    Some(history.metadata_version),
                    history.agents.text,
                    Some(history.agents.version),
                    history.transcript.text,
                    Some(history.transcript.version),
                )
            }
            None => (
                json!({"schemaVersion":3,"id":record.conversation_id,"createdAt":created,"status":"active"}),
                None,
                String::new(),
                None,
                String::new(),
                None,
            ),
        };
    let user = authored.unwrap_or_else(|| json!({"schemaVersion":3,"id":record.input.request_id,"type":"message","createdAt":created,"role":"user","text":record.input.prompt,"provenance":{"originClass":"owner","sourceKind":"user-message"}}));
    let mut slots = Vec::new();
    let mut bindings = agents;
    let count = group.map_or(1, |g| g.deliveries.len());
    for index in 0..count {
        let binding = group.map_or_else(
            || Uuid::new_v4().to_string(),
            |g| g.deliveries[index].agent_binding_id.to_string(),
        );
        let target = group.map_or(record.input.target_id.as_str(), |g| {
            g.input.turns[index].target_id.as_str()
        });
        let mut agent = json!({"schemaVersion":3,"id":binding,"type":"binding.created","createdAt":created,"runtime":"acp","agent":target});
        let delivery = group.map(|g| {
            agent["participant"] = g.input.dispatch["deliveries"][index]["recipient"].clone();
            agent["nativeSessionId"] = g.deliveries[index].session_id.to_string().into();
            g.input.dispatch["deliveries"][index].clone()
        });
        bindings.push_str(&line(&agent));
        let assistant = json!({"schemaVersion":3,"id":Uuid::new_v4(),"type":"message","createdAt":created,"parentId":user["id"],"agentBindingId":binding,"role":"assistant","text":"","provenance":{"originClass":"agent","sourceKind":"assistant-message"}});
        slots.push(Slot {
            assistant,
            delivery,
            begin: Vec::new(),
            terminal: None,
            outcome: None,
            receipt: None,
            finished: false,
        });
    }
    metadata["updatedAt"] = created.clone().into();
    metadata["activeAgentBindingId"] = slots[0].assistant["agentBindingId"].clone();
    let dispatch = group.map(|g| g.input.dispatch.clone());
    if let Some(dispatch) = &dispatch {
        metadata["pendingDispatch"] = dispatch.clone();
        for slot in &mut slots {
            slot.begin.push(
                crate::turns::participant_capture::delivery_entry(
                    dispatch,
                    slot.delivery.as_ref().expect("participant"),
                    &created,
                )
                .map_err(|e| super::super::participants::invalid(&e.message))?,
            );
        }
    }
    let metadata_version = work.write_text(
        &format!("{directory}/metadata.yaml"),
        &line(&metadata),
        &expected(metadata_expected),
    )?;
    let agents_version = work.write_text(
        &format!("{directory}/agents.jsonl"),
        &bindings,
        &expected(agents_expected),
    )?;
    let mut shared = Publication {
        directory,
        metadata,
        metadata_version,
        agents_version,
        transcript_version: transcript_expected.unwrap_or_default(),
        prior,
        user,
        slots,
        dispatch,
        blocked: false,
    };
    let text = shared.text();
    shared.transcript_version = work.write_text(
        &format!("{}/transcript.jsonl", shared.directory),
        &text,
        &expected(
            (!shared.transcript_version.is_empty()).then(|| shared.transcript_version.clone()),
        ),
    )?;
    let shared = Arc::new(Mutex::new(shared));
    Ok((0..count)
        .map(|index| Conversation {
            shared: shared.clone(),
            index,
        })
        .collect())
}
