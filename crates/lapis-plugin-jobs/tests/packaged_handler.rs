//! Explicit consumer acceptance for an exact packaged handler and broker fixture.
use lapis_plugin_jobs::{ExecutionLimits, JobHost, execute};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

struct Host {
    fixture: Value,
    calls: Mutex<Vec<Value>>,
    page: Mutex<usize>,
}
impl JobHost for Host {
    fn invoke(&self, action: &str, input: Value) -> Result<Value, String> {
        self.calls
            .lock()
            .unwrap()
            .push(json!({"action":action,"input":input}));
        match action {
            "cancelled" => Ok(json!(false)),
            "state.load" => Ok(self.fixture["state"].clone()),
            "metadata.query" => {
                let mut index = self.page.lock().unwrap();
                let page = self.fixture["pages"]
                    .as_array()
                    .and_then(|pages| pages.get(*index))
                    .cloned()
                    .ok_or("Unexpected metadata query")?;
                *index += 1;
                Ok(page)
            }
            "state.save" | "notifications.notify" => Ok(Value::Null),
            _ => Err(format!("Undeclared broker call: {action}")),
        }
    }
    fn cancelled(&self) -> bool {
        false
    }
}

#[test]
#[ignore = "Requires exact packaged handler and broker fixture artifacts"]
fn packaged_handler_obeys_the_headless_engine_and_expected_broker_contract() {
    let source = std::fs::read_to_string(
        std::env::var("LAPIS_PLUGIN_JOB_MODULE").expect("Select packaged handler"),
    )
    .unwrap();
    let fixture: Value = serde_json::from_slice(
        &std::fs::read(std::env::var("LAPIS_PLUGIN_JOB_FIXTURE").expect("Select broker fixture"))
            .unwrap(),
    )
    .unwrap();
    let expected = fixture["expected"].clone();
    let context = fixture["context"].clone();
    let host = Arc::new(Host {
        fixture,
        calls: Mutex::new(Vec::new()),
        page: Mutex::new(0),
    });
    execute(&source, &context, host.clone(), ExecutionLimits::default()).unwrap();
    let mutations: Vec<_> = host
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| {
            matches!(
                call["action"].as_str(),
                Some("state.save" | "notifications.notify")
            )
        })
        .cloned()
        .collect();
    assert_eq!(json!(mutations), expected);
}
