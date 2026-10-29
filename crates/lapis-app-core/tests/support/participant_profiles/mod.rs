#[path = "../native_turn/mod.rs"]
#[allow(dead_code)]
mod native;
use lapis_app_core::{
    services::{AgentPolicy, ParticipantMessageInput, Runtime, TurnStorage},
    vault::VaultRoot,
};
use lapis_app_protocol::{
    participants::ParticipantProfile, queues::QueueSelection, turns::TurnTarget,
};
pub use native::Fixture;
use serde_json::json;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct Declared {
    native: Arc<native::Policy>,
    pub profiles: Mutex<Vec<(String, QueueSelection)>>,
}
impl AgentPolicy for Declared {
    fn targets(&self, _: Uuid, _: &VaultRoot) -> lapis_ai_service::Result<Vec<TurnTarget>> {
        Ok(vec![TurnTarget {
            id: "fixture".into(),
            label: "Opaque runtime".into(),
            default_mode: "plan".into(),
            queue_selection: None,
        }])
    }
    fn participant_profiles(
        &self,
        _: Uuid,
        _: &VaultRoot,
    ) -> lapis_ai_service::Result<Vec<(String, QueueSelection)>> {
        Ok(self.profiles.lock().unwrap().clone())
    }
    fn resolve(
        &self,
        vault: Uuid,
        scope: &VaultRoot,
        target: &str,
    ) -> lapis_ai_service::Result<lapis_ai_service::Launch> {
        self.native.resolve(vault, scope, target)
    }
}
pub fn runtime(f: &Fixture) -> (Arc<Runtime>, Arc<Declared>) {
    let policy = Arc::new(Declared {
        native: f.policy.clone(),
        profiles: Mutex::new(["writer", "reviewer"].into_iter().map(|id| {
            let token: String = id.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
            ("fixture".into(), serde_json::from_value(json!({"recipient":{"id":id,"label":id,"token":format!("jj-store-agent:{token};")},"target":{"id":id,"agent":"codex","model":{"provider":"codex","model":format!("model-{id}")},"thinking":"default"}})).unwrap())
        }).collect()),
    });
    let storage = TurnStorage::new(
        &f.profile,
        policy.clone(),
        tokio::runtime::Handle::current(),
    )
    .unwrap();
    let runtime = Arc::new(Runtime::default().with_turns(storage));
    runtime.admit(f.vault, &f.root).unwrap();
    (runtime, policy)
}
pub fn input(f: &Fixture, profiles: Vec<ParticipantProfile>) -> ParticipantMessageInput {
    let parent = Uuid::new_v4().to_string();
    let recipients: Vec<_> = profiles.iter().map(|p| &p.selection.recipient).collect();
    ParticipantMessageInput {
        acceptance_id: Uuid::new_v4(),
        user: json!({"schemaVersion":3,"id":parent,"type":"message","role":"user","text":"Authored café 📝","createdAt":"2026-10-27T04:30:00.000Z","recipients":recipients}),
        dispatch: json!({"version":1,"id":Uuid::new_v4(),"userEntryId":parent,"startedAt":"2026-10-27T04:30:00.000Z","deliveries":recipients.iter().map(|r|json!({"id":Uuid::new_v4(),"recipient":r,"state":"pending"})).collect::<Vec<_>>()}),
        turns: profiles
            .iter()
            .map(|p| {
                let mut t = f.input();
                t.context = None;
                t.target_id = p.target_id.clone();
                t.options = p.options.clone();
                t
            })
            .collect(),
        profiles: Some(profiles),
    }
}
pub async fn shutdown(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
