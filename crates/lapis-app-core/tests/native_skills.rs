use lapis_app_core::{context::skill_catalog::*, services::Result as NativeResult};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;

fn reference() -> Value {
    serde_json::from_str(include_str!("fixtures/ai-context/skills.json")).unwrap()
}
fn outcome<T: Serialize>(result: SkillResult<T>) -> Value {
    match result {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"error":error}),
    }
}

struct Reader {
    files: HashMap<String, String>,
    lists: HashMap<String, Vec<String>>,
    reads: Vec<Value>,
}
impl Reader {
    fn new(input: &Value) -> Self {
        Self {
            files: serde_json::from_value::<Vec<(String, String)>>(input["files"].clone())
                .unwrap()
                .into_iter()
                .collect(),
            lists: serde_json::from_value::<Vec<(String, Vec<String>)>>(input["lists"].clone())
                .unwrap()
                .into_iter()
                .collect(),
            reads: Vec::new(),
        }
    }
}
impl SkillReader for Reader {
    fn list(&mut self, prefix: &str) -> NativeResult<Vec<String>> {
        self.reads
            .push(json!({"glob":format!("{prefix}/**/SKILL.md")}));
        Ok(self.lists.get(prefix).cloned().unwrap_or_default())
    }
    fn read(&mut self, path: &str) -> NativeResult<String> {
        self.reads.push(json!({"read":path}));
        Ok(self.files.get(path).expect("source fixture file").clone())
    }
}

#[test]
fn skill_parser_and_versions_match_complete_source_results() {
    for (index, case) in reference()["parse"].as_array().unwrap().iter().enumerate() {
        let content = case["content"].as_str().unwrap();
        let input = serde_json::from_value(case["input"].clone()).unwrap();
        assert_eq!(
            skill_content_version(content).unwrap(),
            case["version"].as_str().unwrap(),
            "version case{index}"
        );
        assert_eq!(
            outcome(parse_skill(content, &input)),
            case["expected"],
            "parser case{index}"
        );
    }
}

#[test]
fn skill_discovery_matches_complete_source_catalogs_diagnostics_and_reads() {
    for (index, case) in reference()["discovery"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let input = &case["input"];
        let context = serde_json::from_value(input["context"].clone()).unwrap();
        let sources = serde_json::from_value(input["sources"].clone()).unwrap();
        let mut reader = Reader::new(input);
        assert_eq!(
            outcome(discover_skills(&context, &sources, &mut reader)),
            case["expected"],
            "discovery case{index}"
        );
        assert_eq!(json!(reader.reads), case["reads"], "discovery reads{index}");
    }
}

#[test]
fn skill_registry_preserves_complete_version_snapshot_and_activation_traces() {
    for (index, case) in reference()["registry"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let input = &case["input"];
        let context = serde_json::from_value(input["seed"]["context"].clone()).unwrap();
        let sources = serde_json::from_value(input["seed"]["sources"].clone()).unwrap();
        let mut reader = Reader::new(&input["seed"]);
        let mut registry = SkillRegistry::default();
        let mut snapshots: Vec<Value> = Vec::new();
        let undefined = json!({"ok":true,"value":{"$undefined":true}});
        for (step_index, step) in input["steps"].as_array().unwrap().iter().enumerate() {
            let actual = match step["op"].as_str().unwrap() {
                "snapshot" => {
                    let nonce = format!("00000000-0000-4000-8000-{:012}", snapshots.len() + 1);
                    let result = registry.snapshot(
                        &context,
                        &sources,
                        &mut reader,
                        &nonce,
                        "2026-10-27T00:00:00.000Z",
                    );
                    if let Ok(value) = &result {
                        snapshots.push(serde_json::to_value(value).unwrap());
                    }
                    outcome(result)
                }
                "discover" => outcome(registry.discover(&context, &sources, &mut reader)),
                "modify" => {
                    reader.files.insert(
                        step["path"].as_str().unwrap().into(),
                        step["content"].as_str().unwrap().into(),
                    );
                    undefined.clone()
                }
                "invalidate" => {
                    registry.invalidate();
                    undefined.clone()
                }
                "getLoaded" => registry
                    .get_loaded(step["name"].as_str().unwrap())
                    .map_or_else(|| undefined.clone(), |v| json!({"ok":true,"value":v})),
                "diagnostics" => json!({"ok":true,"value":registry.diagnostics()}),
                "loadSaved" => {
                    let entry =
                        &snapshots[step["snapshot"].as_u64().unwrap() as usize]["skills"][0];
                    outcome(registry.load(
                        entry["skillId"].as_str().unwrap(),
                        entry["version"].as_str().unwrap(),
                    ))
                }
                "load" => {
                    let version = step["version"].as_str().unwrap_or_else(|| {
                        snapshots[step["versionFrom"].as_u64().unwrap() as usize]["skills"][0]
                            ["version"]
                            .as_str()
                            .unwrap()
                    });
                    outcome(registry.load(step["id"].as_str().unwrap(), version))
                }
                "activate" => outcome(registry.activate(
                    &context,
                    &sources,
                    &mut reader,
                    &serde_json::from_value(step.clone()).unwrap(),
                )),
                _ => panic!("unknown source operation"),
            };
            assert_eq!(
                actual, case["expected"][step_index],
                "registry{index} step{step_index}"
            );
        }
        assert_eq!(json!(reader.reads), case["reads"], "registry reads{index}");
    }
}
