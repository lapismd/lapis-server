use lapis_app_core::services::{
    Runtime,
    daily::{DailyDocumentOperationError, DailyDocumentSettings, DailyFormatContext},
};
use lapis_app_protocol::ErrorCode;
use serde_json::{Value, json};
use std::fs;
use uuid::Uuid;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/daily-provider-en-gb.json")).unwrap()
}
fn context(value: &Value) -> DailyFormatContext {
    serde_json::from_value(json!({"locale":value["locale"]["locale"],"firstDay":value["weekSettings"]["firstDay"],"minimalDays":value["weekSettings"]["minimalDays"]})).unwrap()
}

fn corpus_cases() -> [(Value, Vec<Value>); 2] {
    [
        (
            fixture(),
            serde_json::from_str(include_str!("fixtures/daily-provider-en-gb-io.json")).unwrap(),
        ),
        (
            serde_json::from_str(include_str!(
                "fixtures/daily-provider-en-us-minimal-one.json"
            ))
            .unwrap(),
            serde_json::from_str(include_str!(
                "fixtures/daily-provider-en-us-minimal-one-io.json"
            ))
            .unwrap(),
        ),
    ]
}

#[test]
fn daily_qualified_locales_physical_io_uses_retained_filenames_full_seed_bytes_and_canonical_reuse()
{
    for (fixture, cases) in corpus_cases() {
        let context = context(&fixture);
        assert_eq!(cases.len(), 8);
        for case in cases {
            let expected_path = case["expected"]["result"]["value"].as_str().unwrap();
            let seed = case["expected"]["files"][expected_path].as_str().unwrap();
            let root = tempfile::tempdir().unwrap();
            let runtime = Runtime::default();
            let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
            let work = vault.begin_work().unwrap();
            let settings = DailyDocumentSettings {
                folder: case["configuration"]["dailyNotes.folder"]
                    .as_str()
                    .unwrap()
                    .into(),
                date_format: case["configuration"]["dailyNotes.dateFormat"]
                    .as_str()
                    .unwrap()
                    .into(),
            };
            let snapshot = work
                .ensure_default_daily_document("2026-08-21", &settings, &context)
                .unwrap();
            assert_eq!(snapshot.path, expected_path);
            assert_eq!(snapshot.text, seed);
            assert_eq!(
                fs::read_to_string(root.path().join(&snapshot.path)).unwrap(),
                seed
            );
            assert_eq!(
                work.read_text(&snapshot.path).unwrap().version,
                snapshot.version
            );
            let reused = work
                .ensure_default_daily_document(
                    "2026-08-21",
                    &DailyDocumentSettings {
                        folder: "../invalid".into(),
                        date_format: "".into(),
                    },
                    &context,
                )
                .unwrap();
            assert_eq!(reused.path, snapshot.path);
            assert_eq!(reused.version, snapshot.version);
            assert_eq!(reused.text, seed);
        }
    }
}

#[test]
fn daily_qualified_locales_dirty_and_occupied_targets_retain_existing_guards_and_source_errors() {
    for (fixture, _) in corpus_cases() {
        let context = context(&fixture);
        for detached in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let runtime = Runtime::default();
            let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
            let mut client = Some(vault.attach().unwrap());
            let editor = client
                .as_ref()
                .unwrap()
                .open_editor("daily/2026-08-21.md")
                .unwrap();
            client.as_ref().unwrap().mark_dirty(editor, 1).unwrap();
            if detached {
                drop(client.take());
            }
            let work = vault.begin_work().unwrap();
            let error = work
                .ensure_default_daily_document(
                    "2026-08-21",
                    &DailyDocumentSettings::default(),
                    &context,
                )
                .unwrap_err();
            assert!(
                matches!(error,DailyDocumentOperationError::Service(ref error) if error.code()==ErrorCode::Busy)
            );
            assert!(!root.path().join("daily").exists());
            let recovered = client.take().unwrap_or_else(|| {
                let recovered = vault.attach().unwrap();
                recovered.recover_editor(editor).unwrap();
                recovered
            });
            recovered.discard_editor(editor).unwrap();
            work.ensure_default_daily_document(
                "2026-08-21",
                &DailyDocumentSettings::default(),
                &context,
            )
            .unwrap();
        }
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("daily")).unwrap();
        fs::write(root.path().join("daily/2026-08-21.md"), "# Unrelated\n").unwrap();
        let runtime = Runtime::default();
        let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
        let work = vault.begin_work().unwrap();
        let DailyDocumentOperationError::Policy(error) = work
            .ensure_default_daily_document(
                "2026-08-21",
                &DailyDocumentSettings::default(),
                &context,
            )
            .unwrap_err()
        else {
            panic!("policy identity")
        };
        let expected = fixture["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == "occupied unrelated target")
            .unwrap()["expected"]["result"]["error"]
            .clone();
        assert_eq!(serde_json::to_value(error).unwrap(), expected);
        assert_eq!(
            fs::read_to_string(root.path().join("daily/2026-08-21.md")).unwrap(),
            "# Unrelated\n"
        );
    }
}
