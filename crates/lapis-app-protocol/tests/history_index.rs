use lapis_app_protocol::history_index::*;
use serde_json::json;
use ts_rs::{Config, TS};

#[test]
fn history_index_contract_round_trips_retained_query_and_context_shapes() {
    for frame in [
        json!({"kind":"start"}),
        json!({"kind":"status"}),
        json!({"kind":"context","commit":"abcd","path":"Note.md","start":0,"end":2}),
        json!({"kind":"query","search":{
            "expression":{"type":"and","clauses":[
                {"type":"term","field":"path","value":"Note.md"},
                {"type":"not","operand":{"type":"term","field":null,"value":"private"}}
            ]},"case_sensitive":true,"cursor":null,"limit":20
        }}),
    ] {
        let decoded: HistoryIndexCommand = serde_json::from_value(frame.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), frame);
    }
    let cfg = Config::default();
    let generated = HistoryIndexCommand::decl(&cfg);
    for command in ["start", "status", "query", "context"] {
        assert!(generated.contains(&format!("\"{command}\"")));
    }
    assert!(!generated.contains("step"));
    assert!(
        HistoryIndexValue::decl(&cfg)
            .contains("HistoryIndexProgress | HistoryIndexPage | HistoryIndexContext")
    );
    let context = json!({"text":"saved","start":0,"end":1,"before":false,"after":true});
    let value: HistoryIndexValue = serde_json::from_value(context.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), context);
}

#[test]
fn history_index_contract_rejects_renderer_scheduling_and_nested_authority() {
    assert!(serde_json::from_value::<HistoryIndexCommand>(json!({"kind":"step"})).is_err());
    let query = json!({"kind":"query","search":{
        "expression":{"type":"term","field":null,"value":"saved"},
        "case_sensitive":false,"cursor":null,"limit":100
    }});
    for field in [
        "root",
        "state",
        "index",
        "exclusions",
        "caller",
        "client_id",
    ] {
        for depth in 0..3 {
            let mut frame = query.clone();
            let target = match depth {
                0 => &mut frame,
                1 => &mut frame["search"],
                _ => &mut frame["search"]["expression"],
            };
            target[field] = json!("outside");
            assert!(serde_json::from_value::<HistoryIndexCommand>(frame).is_err());
        }
    }
    let schema = serde_json::to_value(schemars::schema_for!(HistoryIndexCommand)).unwrap();
    for variant in schema["oneOf"].as_array().unwrap() {
        assert_eq!(variant["additionalProperties"], false);
    }
}
