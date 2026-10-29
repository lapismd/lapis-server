use lapis_app_core::services::Runtime;
use lapis_app_protocol::ExpectedVersion;
use lapis_task_services::scan_task_occurrence_observations;
use serde_json::Value;
use std::fs;
use uuid::Uuid;

fn fixtures() -> Value {
    serde_json::from_str(include_str!("fixtures/task-writes.json")).unwrap()
}

#[test]
fn admitted_work_persists_complete_qualified_task_transformations() {
    let fixtures = fixtures();
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    for family in ["mutation", "materialization", "completion"] {
        let case = &fixtures["cases"][family];
        let path = case["path"].as_str().unwrap();
        let raw = case["raw"].as_str().unwrap();
        fs::create_dir_all(root.path().join(path).parent().unwrap()).unwrap();
        fs::write(root.path().join(path), raw).unwrap();
        let snapshot = work.read_text(path).unwrap();
        let expected = ExpectedVersion::Version {
            version: snapshot.version,
        };
        let result = match family {
            "mutation" => work.mutate_task(
                path,
                &expected,
                &serde_json::from_value(case["actions"][0].clone()).unwrap(),
            ),
            "materialization" => work.materialize_task_occurrence(
                path,
                &expected,
                &serde_json::from_value(case["input"].clone()).unwrap(),
            ),
            _ => work.complete_task_occurrence(
                path,
                &expected,
                &scan_task_occurrence_observations(raw, path)[0],
                &serde_json::from_value(case["completion"].clone()).unwrap(),
            ),
        }
        .unwrap();
        assert_eq!(
            result.text,
            case["expected"]["value"]["raw"].as_str().unwrap(),
            "{family}"
        );
        assert_eq!(work.read_text(path).unwrap().version, result.version);
        assert_eq!(
            fs::read_to_string(root.path().join(path)).unwrap(),
            result.text
        );
    }
}
