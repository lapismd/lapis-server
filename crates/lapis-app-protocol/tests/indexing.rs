use lapis_app_protocol::indexing::*;
use serde_json::json;
use ts_rs::{Config, TS};

#[test]
fn indexing_declarations_round_trip_without_callbacks_or_host_roots() {
    let input = json!({"kind":"register_projection","contribution":{"projection_id":"docs/note","owner_plugin_id":"docs","schema_version":1.0,"configuration_hash":"opaque","path_prefix":"docs"}});
    let command: IndexingCommand = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(serde_json::to_value(command).unwrap(), input);
    let mut callback = input;
    callback["contribution"]["project"] = json!("function(){}");
    assert!(serde_json::from_value::<IndexingCommand>(callback).is_err());
    assert!(
        serde_json::from_value::<IndexingCommand>(
            json!({"kind":"select_metadata","root":"/outside"})
        )
        .is_err()
    );
    let declarations = IndexingCommand::decl(&Config::default());
    for kind in [
        "select_metadata",
        "select_search",
        "disable_search",
        "register_processor",
        "register_projection",
        "disable_projection",
        "refresh",
        "status",
    ] {
        assert!(declarations.contains(kind));
    }
}

#[test]
fn search_declarations_are_bounded_data_and_reject_callback_and_path_fields() {
    let input = json!({"kind":"select_search","selection":{"providers":[{"provider_id":"search:canvas","version":"1","priority":0.0}],"chunking":{"targetChars":256.0,"breakpointWindowChars":32.0,"breakpointDecay":0.5}}});
    let command: IndexingCommand = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(serde_json::to_value(command).unwrap(), input);
    for key in ["extract", "matches", "root"] {
        let mut invalid = input.clone();
        invalid["selection"]["providers"][0][key] = json!("forbidden");
        assert!(
            serde_json::from_value::<IndexingCommand>(invalid).is_err(),
            "{key}"
        );
    }
    let schema =
        serde_json::to_value(schemars::schema_for!(search::SearchIndexingSelection)).unwrap();
    assert_eq!(schema["properties"]["providers"]["maxItems"], 64);
}
