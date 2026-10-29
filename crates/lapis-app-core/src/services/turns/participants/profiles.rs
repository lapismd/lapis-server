use super::super::{Client, Owner, Result, Work};
use super::{ParticipantMessageInput, invalid};
use crate::turns::queue_capture;
use lapis_app_protocol::{
    participants::{PARTICIPANT_PROFILE_LIMIT, ParticipantProfile},
    turns::{TurnInput, TurnOptions},
};
use serde_json::json;
use std::collections::HashSet;
use uuid::Uuid;

impl Client {
    /// Read-only metadata from the same native installation policy as turn targets.
    pub fn participant_profiles(&self) -> Result<Vec<ParticipantProfile>> {
        let (owner, work) = self.turn_owner()?;
        {
            let data = owner.data()?;
            owner.require_enabled(&data)?;
        }
        read(&owner, &work)
    }
}
pub(super) fn read(owner: &Owner, work: &Work) -> Result<Vec<ParticipantProfile>> {
    let scope = work.vault.lock()?.root.try_clone()?;
    let declared = owner
        .storage
        .policy
        .participant_profiles(work.vault.id, &scope)
        .map_err(|_| invalid("Native participant declarations are unavailable"))?;
    if declared.is_empty() {
        return Ok(Vec::new());
    }
    if declared.len() > PARTICIPANT_PROFILE_LIMIT {
        return Err(invalid("Native participant profile limit exceeded"));
    }
    let targets = owner
        .storage
        .policy
        .targets(work.vault.id, &scope)
        .map_err(|_| invalid("Native participant runtime targets are unavailable"))?;
    if targets.len() > 32 {
        return Err(invalid("Native runtime target limit exceeded"));
    }
    let mut ids = HashSet::new();
    declared
        .into_iter()
        .map(|(target_id, selection)| {
            queue_capture::validate_declared_selection(&selection)
                .map_err(|e| invalid(&e.message))?;
            if !ids.insert(selection.recipient.id.clone()) {
                return Err(invalid("Native participant identity is ambiguous"));
            }
            let matches: Vec<_> = targets.iter().filter(|t| t.id == target_id).collect();
            if matches.len() != 1 {
                return Err(invalid(
                    "Native participant runtime target is unavailable or ambiguous",
                ));
            }
            let mut target = json!(selection.target);
            target["executionMode"] = json!(
                selection
                    .target
                    .execution_mode
                    .as_deref()
                    .unwrap_or(&matches[0].default_mode)
            );
            if let Some(thinking) = selection
                .target
                .thinking
                .as_ref()
                .or(selection.thinking.as_ref())
            {
                target["thinking"] = json!(thinking);
            }
            let provider = json!(selection.target.agent);
            let frame = queue_capture::configuration_frame(
                &target,
                provider.as_str().expect("typed provider"),
            )
            .map_err(|e| invalid(&e.message))?;
            let input = crate::turns::normalize(TurnInput {
                request_id: Uuid::from_u128(0x00000000000040008000000000000001),
                target_id: target_id.clone(),
                scope_dir: String::new(),
                prompt: "Validate declared native profile".into(),
                context: None,
                continuation: None,
                options: TurnOptions {
                    mode: frame["mode"].as_str().expect("typed mode").into(),
                    model: frame["model"].as_str().map(String::from),
                    effort: frame["effort"].as_str().map(String::from),
                    ..Default::default()
                },
            })?;
            Ok(ParticipantProfile {
                target_id,
                selection,
                options: input.options,
            })
        })
        .collect()
}
/// Stored captures validate independently of later installation changes.
pub(super) fn captured(input: &ParticipantMessageInput) -> Result<()> {
    let Some(profiles) = &input.profiles else {
        return Ok(());
    };
    if profiles.len() != input.turns.len() {
        return Err(invalid(
            "Participant profiles do not match their deliveries",
        ));
    }
    for (index, (profile, turn)) in profiles.iter().zip(&input.turns).enumerate() {
        queue_capture::validate_declared_selection(&profile.selection)
            .map_err(|e| invalid(&e.message))?;
        if profile.target_id != turn.target_id
            || profile.options != turn.options
            || json!(profile.selection.recipient)
                != input.dispatch["deliveries"][index]["recipient"]
        {
            return Err(invalid(
                "Participant selected-profile capture changed its delivery",
            ));
        }
    }
    Ok(())
}
pub(super) fn admit(owner: &Owner, work: &Work, input: &ParticipantMessageInput) -> Result<()> {
    let Some(profiles) = &input.profiles else {
        return Ok(());
    };
    let current = read(owner, work)?;
    if profiles
        .iter()
        .any(|profile| !current.iter().any(|p| p == profile))
    {
        return Err(invalid(
            "The declared participant profile changed before acceptance",
        ));
    }
    Ok(())
}
