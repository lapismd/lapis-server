use lapis_app_core::{
    context::bootstrap::{BootstrapInput, prepare_bootstrap},
    services::Runtime,
};
use lapis_app_protocol::ErrorCode;
use serde_json::{Value, json};
use std::fs;
use uuid::Uuid;

#[test]
fn native_bootstrap_matches_complete_source_results_and_read_order() {
    let reference: Value =
        serde_json::from_str(include_str!("fixtures/ai-context/bootstrap.json")).unwrap();
    for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
        let input: BootstrapInput = serde_json::from_value(case["input"].clone()).unwrap();
        let mut reads = Vec::new();
        let mut reader = |path: &str| {
            reads.push(path.to_owned());
            Ok(case["files"][path].as_str().map(str::to_owned))
        };
        let selected = (case["read"] == true).then_some(&mut reader as &mut dyn FnMut(&str) -> _);
        let actual = prepare_bootstrap(&input, selected, || false);
        if case["expected"]["nativeUnicodeRefusal"] == true {
            assert!(
                serde_json::from_str::<Value>(case["expected"]["sourceWire"].as_str().unwrap())
                    .is_err()
            );
            assert_eq!(actual.unwrap_err().code(), ErrorCode::InvalidRequest);
        } else {
            let actual = match actual {
                Ok(value) => json!({"ok":true,"value":value}),
                Err(error) => {
                    assert_eq!(error.code(), ErrorCode::InvalidRequest);
                    json!({"ok":false,"error":{"name":"Error","message":error.to_string()}})
                }
            };
            assert_eq!(actual, case["expected"], "bootstrap case{index}");
        }
        assert_eq!(
            json!(reads),
            case["reads"],
            "bootstrap read order case{index}"
        );
    }
}

#[test]
fn admitted_bootstrap_uses_current_ancestor_rules_after_client_detach() {
    let root = tempfile::tempdir().unwrap();
    for dir in [".lapis", "Projects/.lapis", "Projects/Lapis/.lapis"] {
        fs::create_dir_all(root.path().join(dir)).unwrap();
    }
    fs::write(root.path().join(".lapis/AGENTS.md"), "Root rules").unwrap();
    fs::write(
        root.path().join("Projects/.lapis/AGENTS.md"),
        "Project rules",
    )
    .unwrap();
    fs::write(root.path().join("Projects/Lapis/.lapis/AGENTS.md"), "Old").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    let work = vault.begin_work().unwrap();
    drop(client);
    fs::write(
        root.path().join("Projects/Lapis/.lapis/AGENTS.md"),
        "Fresh café 📝",
    )
    .unwrap();
    let value = work
        .bootstrap_context(
            &BootstrapInput {
                scope_dir: "Projects/Lapis".into(),
                ..Default::default()
            },
            || false,
        )
        .unwrap();
    assert!(value.text.contains("Fresh café 📝"));
    assert!(value.text.find("Root rules").unwrap() < value.text.find("Project rules").unwrap());
    assert!(!value.text.contains(root.path().to_str().unwrap()));
    assert_eq!(vault.status().unwrap().clients, 0);
    assert_eq!(
        fs::read_to_string(root.path().join("Projects/Lapis/.lapis/AGENTS.md")).unwrap(),
        "Fresh café 📝"
    );
}

#[test]
fn native_bootstrap_omits_path_bearing_rules_and_refuses_path_bearing_labels() {
    let mut reader = |_: &str| Ok(Some("Read /Users/private/file".to_owned()));
    let value = prepare_bootstrap(&BootstrapInput::default(), Some(&mut reader), || false).unwrap();
    assert_eq!(
        value.bootstrap.folder_instructions[0].omitted.as_deref(),
        Some("path-bearing")
    );
    assert!(!value.text.contains("/Users/private"));
    let input = BootstrapInput {
        workspace_label: Some("/Users/private".into()),
        ..Default::default()
    };
    let error = prepare_bootstrap(&input, None, || false).unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidRequest);
    assert_eq!(
        error.to_string(),
        "Session bootstrap must not include host filesystem paths."
    );
}
