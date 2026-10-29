use super::*;
use crate::services::Runtime;
use serde_json::Value;
use std::{fs, process::Command};

#[test]
#[ignore = "isolated Task crash-process helper"]
fn crash_peer() {
    let root = std::env::var_os("LAPIS_TASK_CRASH_ROOT").unwrap();
    let profile = std::env::var_os("LAPIS_TASK_CRASH_PROFILE").unwrap();
    let id = Uuid::parse_str(&std::env::var("LAPIS_TASK_CRASH_VAULT").unwrap()).unwrap();
    let request =
        serde_json::from_str(&std::env::var("LAPIS_TASK_CRASH_REQUEST").unwrap()).unwrap();
    let policy = serde_json::from_str(&std::env::var("LAPIS_TASK_CRASH_POLICY").unwrap()).unwrap();
    let vault = Runtime::default().admit(id, Path::new(&root)).unwrap();
    let mut journal = TaskLifecycleJournal::open(Path::new(&profile), &vault).unwrap();
    journal
        .submit_with(vault.begin_work().unwrap(), request, &policy, |stage| {
            if stage == Checkpoint::Advanced {
                std::process::exit(73);
            }
            Ok(())
        })
        .unwrap();
    panic!("crash checkpoint was not reached");
}

#[test]
fn actual_process_loss_after_original_publication_never_advances_a_task_twice() {
    use std::os::unix::fs::MetadataExt;
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/task-lifecycle.json"
    )))
    .unwrap();
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "complete / schedule / count")
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("vault");
    let profile = base.join("profile");
    fs::create_dir_all(root.join("tasks")).unwrap();
    fs::create_dir(&profile).unwrap();
    let task = root.join("tasks/Read.md");
    fs::write(&task, case["raw"].as_str().unwrap()).unwrap();
    let id = Uuid::new_v4();
    let vault = Runtime::default().admit(id, &root).unwrap();
    let request = TaskLifecycleRequest {
        request_id: Uuid::new_v4(),
        path: "tasks/Read.md".into(),
        expected_version: vault
            .begin_work()
            .unwrap()
            .read_text("tasks/Read.md")
            .unwrap()
            .version,
        action: serde_json::from_value(case["actions"][0].clone()).unwrap(),
    };
    let policy = TaskLifecyclePolicy {
        day_parts: serde_json::from_value(case["settings"].clone()).unwrap(),
        daily: DailyDocumentSettings::default(),
        format: DailyFormatContext {
            locale: "en-US".into(),
            first_day: 7,
            minimal_days: 4,
        },
    };
    vault.close().unwrap();
    drop(vault);
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "services::tasks::lifecycle::recovery::process_tests::crash_peer",
            "--nocapture",
        ])
        .env("LAPIS_TASK_CRASH_ROOT", &root)
        .env("LAPIS_TASK_CRASH_PROFILE", &profile)
        .env("LAPIS_TASK_CRASH_VAULT", id.to_string())
        .env(
            "LAPIS_TASK_CRASH_REQUEST",
            serde_json::to_string(&request).unwrap(),
        )
        .env(
            "LAPIS_TASK_CRASH_POLICY",
            serde_json::to_string(&policy).unwrap(),
        )
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(73),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&task).unwrap(),
        case["expected"]["attempts"][0]["taskRaw"]
    );
    let inode = fs::metadata(&task).unwrap().ino();
    let daily = fs::read(root.join("daily/2026-08-20.md")).unwrap();
    let record = profile
        .join(id.to_string())
        .join(format!("{}.json", request.request_id));
    let before = fs::read(&record).unwrap();
    let vault = Runtime::default().admit(id, &root).unwrap();
    let mut journal = TaskLifecycleJournal::open(&profile, &vault).unwrap();
    let status = journal
        .inspect(&vault.begin_work().unwrap(), request.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(status.disposition, TaskLifecycleDisposition::Completed);
    assert!(status.last_failure.is_none());
    assert_eq!(fs::read(&record).unwrap(), before);
    journal
        .retry(vault.begin_work().unwrap(), request.request_id)
        .unwrap();
    assert_eq!(
        fs::read_to_string(&task).unwrap(),
        case["expected"]["attempts"][0]["taskRaw"]
    );
    assert_eq!(fs::metadata(&task).unwrap().ino(), inode);
    assert_eq!(fs::read(root.join("daily/2026-08-20.md")).unwrap(), daily);
}
