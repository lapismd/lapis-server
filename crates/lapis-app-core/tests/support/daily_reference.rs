use lapis_app_core::services::daily::{
    DailyDocumentCandidate, DailyDocumentPlan, DailyDocumentProviderRegistry,
    DailyDocumentSettings, DailyFormatContext, format_daily_document_filename, plan_daily_document,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

fn context(value: &Value) -> DailyFormatContext {
    serde_json::from_value(json!({
        "locale": value["locale"]["locale"],
        "firstDay": value["weekSettings"]["firstDay"],
        "minimalDays": value["weekSettings"]["minimalDays"],
    }))
    .unwrap()
}

fn numbers(value: &mut Value) {
    match value {
        Value::Number(number) => {
            *number = serde_json::Number::from_f64(number.as_f64().unwrap()).unwrap()
        }
        Value::Array(values) => values.iter_mut().for_each(numbers),
        Value::Object(values) => values.values_mut().for_each(numbers),
        _ => {}
    }
}

pub fn filenames(fixture: &Value) {
    let context = context(fixture);
    let mut differences = Vec::new();
    for case in fixture["filenames"].as_array().unwrap() {
        let actual = match format_daily_document_filename(
            case["date"].as_str().unwrap(),
            case["format"].as_str().unwrap(),
            &context,
        ) {
            Ok(value) => json!({ "value": value }),
            Err(error) => json!({ "error": error }),
        };
        if actual != case["expected"] {
            differences.push(json!({"date":case["date"], "format":case["format"],
                "actual":actual, "expected":case["expected"]}));
        }
    }
    assert_eq!(fixture["filenames"].as_array().unwrap().len(), 200);
    assert!(differences.is_empty(), "{}", json!(differences));
}

pub fn providers(fixture: &Value) {
    let context = context(fixture);
    let mut differences = Vec::new();
    for case in fixture["providers"].as_array().unwrap() {
        let mut files = case["files"].as_object().cloned().unwrap_or_default();
        let mut candidates = files
            .iter()
            .map(|(path, raw)| DailyDocumentCandidate {
                path: path.clone(),
                frontmatter: case["cached"][path].as_object().cloned().map_or_else(
                    || {
                        lapis_plugin_services::markdown::extract_metadata(raw.as_str().unwrap())
                        ["frontmatter"].clone()
                    },
                    Value::Object,
                ),
            })
            .collect::<Vec<_>>();
        // The actual provider's read trace preserves its supplied Vault inventory
        // order; serde_json's object map intentionally does not preserve it.
        let observed = case["expected"]["trace"].as_array().unwrap();
        candidates.sort_by_key(|candidate| {
            observed
                .iter()
                .position(|entry| entry["kind"] == "cache" && entry["path"] == candidate.path)
        });
        let settings = DailyDocumentSettings {
            folder: case["configuration"]["dailyNotes.folder"]
                .as_str()
                .unwrap_or("daily")
                .into(),
            date_format: case["configuration"]["dailyNotes.dateFormat"]
                .as_str()
                .unwrap_or("yyyy-MM-dd")
                .into(),
        };
        let result = match plan_daily_document(
            case["date"].as_str().unwrap(),
            case["operation"] == "ensure",
            &candidates,
            &settings,
            &context,
        ) {
            Ok(plan) => {
                let path = match plan {
                    Some(DailyDocumentPlan::Existing { path }) => Some(path),
                    Some(DailyDocumentPlan::Create { path, text }) => {
                        files.insert(path.clone(), json!(text));
                        Some(path)
                    }
                    None => None,
                };
                json!({ "value":path, "providerId":"lapis:daily-notes" })
            }
            Err(error) => json!({ "error":error }),
        };
        let actual = json!({ "result":result, "files":files });
        let expected = json!({ "result":case["expected"]["result"],
            "files":case["expected"]["files"] });
        if actual != expected {
            differences.push(json!({ "name":case["name"], "actual":actual, "expected":expected }));
        }
    }
    assert_eq!(fixture["providers"].as_array().unwrap().len(), 45);
    assert!(differences.is_empty(), "{}", json!(differences));
}

pub fn registry(fixture: &Value) {
    let changes = Arc::new(Mutex::new(Vec::<Value>::new()));
    let observed = changes.clone();
    let mut registry = DailyDocumentProviderRegistry::with_observer(move |change| {
        observed
            .lock()
            .unwrap()
            .push(serde_json::to_value(change).unwrap());
    });
    let mut registrations = HashMap::new();
    for expected in fixture["registry"].as_array().unwrap() {
        let action = &expected["action"];
        let result = match action["type"].as_str().unwrap() {
            "register" => {
                let priority = match action["priority"].as_str() {
                    Some("Infinity") => Some(f64::INFINITY),
                    Some("NaN") => Some(f64::NAN),
                    _ => action["priority"].as_f64(),
                };
                let id = action["id"].as_str().unwrap();
                match registry.register(id, priority, id.trim().to_string()) {
                    Ok(registration) => {
                        let id = registration.id().to_string();
                        registrations.insert(id.clone(), registration);
                        json!({"id": id})
                    }
                    Err(error) => json!({"error":error}),
                }
            }
            "dispose" => {
                let id = action["id"].as_str().unwrap();
                registry.dispose(&registrations[id]);
                json!({"disposed":id})
            }
            "resolve" => match registry.resolve() {
                Ok(id) => json!({"selected":id}),
                Err(error) => json!({"error":error}),
            },
            _ => unreachable!(),
        };
        let mut actual = json!({"action":action,"result":result,
            "providers":registry.get_all(),"changes":*changes.lock().unwrap()});
        let mut expected = expected.clone();
        numbers(&mut actual);
        numbers(&mut expected);
        assert_eq!(actual, expected);
    }
}
