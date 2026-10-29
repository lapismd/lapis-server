use lapis_app_core::{
    context::{
        SelectedSkillActivation, SkillManifestEntry, available_skills_manifest,
        prepare_initial_context,
    },
    services::Result,
};
use lapis_app_protocol::ErrorCode;
use serde_json::{Value, json};

fn reference() -> Value {
    serde_json::from_str(include_str!("fixtures/ai-context/initial.json")).unwrap()
}
fn result(value: Result<Option<String>>) -> Value {
    match value {
        Ok(Some(value)) => json!({"ok":true,"value":value}),
        Ok(None) => json!({"ok":true}),
        Err(error) => {
            assert_eq!(error.code(), ErrorCode::InvalidRequest);
            json!({"ok":false,"error":{"name":"Error","message":error.to_string()}})
        }
    }
}

#[test]
fn native_manifest_matches_all_complete_retained_results() {
    for case in reference()["manifest"].as_array().unwrap() {
        let skills: Vec<SkillManifestEntry> =
            serde_json::from_value(case["input"]["skills"].clone()).unwrap();
        assert_eq!(
            result(available_skills_manifest(&skills).map(Some)),
            case["expected"]
        );
    }
}

#[test]
fn native_initial_context_matches_all_complete_retained_results() {
    for (index, case) in reference()["initial"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let input = &case["input"];
        let skills = input.get("skillSnapshot").map(|snapshot| {
            serde_json::from_value::<Vec<SkillManifestEntry>>(snapshot["skills"].clone()).unwrap()
        });
        let activations: Vec<SelectedSkillActivation> = input
            .get("skillActivations")
            .map(|value| serde_json::from_value(value.clone()).unwrap())
            .unwrap_or_default();
        let actual = prepare_initial_context(
            input["metadata"]["sessionBootstrap"].as_str(),
            skills.as_deref(),
            &activations,
        );
        assert_eq!(
            result(actual),
            case["expected"],
            "initial-context case{index}"
        );
    }
}

#[test]
fn native_preparation_retains_owned_text_without_mutating_selected_inputs() {
    let mut activation: SelectedSkillActivation = serde_json::from_value(json!({"skillId":"vault:research","version":"v1","instructions":"Original <instructions>","arguments":"notes"})).unwrap();
    let before = activation.clone();
    let value = prepare_initial_context(Some("Bootstrap"), None, &[activation.clone()])
        .unwrap()
        .unwrap();
    assert_eq!(activation, before);
    activation.instructions = "Changed".into();
    assert!(value.contains("Original &lt;instructions&gt;"));
    assert!(!value.contains("Changed"));
    assert_ne!(
        Some(value),
        prepare_initial_context(Some("Bootstrap"), None, &[activation]).unwrap()
    );
}
