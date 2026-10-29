use lapis_app_core::{
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime, VaultService},
};
use lapis_app_protocol::database::call::{DatabaseCall, DatabaseMethod, DatabaseValue};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

const PATH: &str = "daily/2026-08-20.md";
const HEADER: &str = "---\ntype: daily-note\ndate: 2026-08-20\n---\n";
// Reuse the parser reference's JavaScript numeric domain: JSON 2 and 2.0
// represent the same value. All keys, array positions and other values remain exact.
fn javascript_numbers(mut value: Value) -> Value {
    match &mut value {
        Value::Number(number) => {
            *number = serde_json::Number::from_f64(number.as_f64().unwrap()).unwrap();
        }
        Value::Array(values) => {
            for value in values {
                *value = javascript_numbers(value.take());
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                *value = javascript_numbers(value.take());
            }
        }
        _ => {}
    }
    value
}
fn source(id: &str) -> String {
    format!(
        "{HEADER}- [ ] [Read](../tasks/read.md) <!-- task-occurrence task={id} date=2026-08-20 kind=boolean -->\n"
    )
}
fn runtime(profile: &tempfile::TempDir) -> Arc<Runtime> {
    Arc::new(Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    ))
}
fn write(root: &tempfile::TempDir, text: &str) {
    std::fs::create_dir_all(root.path().join("daily")).unwrap();
    std::fs::write(root.path().join(PATH), text).unwrap();
}
async fn call(
    vault: &Arc<VaultService>,
    method: DatabaseMethod,
    values: Vec<Value>,
) -> lapis_app_core::services::Result<Value> {
    let work = vault.begin_work()?;
    Ok(
        match work
            .call_database(DatabaseCall {
                method,
                args: values
                    .into_iter()
                    .map(|value| DatabaseValue::Value { value })
                    .collect(),
            })
            .await?
        {
            DatabaseValue::Value { value } => value,
            DatabaseValue::Undefined {} => Value::Null,
        },
    )
}
async fn query(vault: &Arc<VaultService>) -> Value {
    call(
        vault,
        DatabaseMethod::QueryProjection,
        vec![json!("tasks/occurrence"), json!({"includeStale": true})],
    )
    .await
    .unwrap()
}
async fn until(mut ready: impl AsyncFnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready().await {
        assert!(
            Instant::now() < deadline,
            "native occurrence reconciliation did not complete"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
async fn settled(vault: &Arc<VaultService>) {
    until(async || vault.indexing_status().unwrap().passes > 0).await;
}
async fn rescan(vault: &Arc<VaultService>, root: &tempfile::TempDir) {
    let passes = vault.indexing_status().unwrap().passes;
    std::fs::write(root.path().join("signal.txt"), Uuid::new_v4().to_string()).unwrap();
    until(async || vault.indexing_status().unwrap().passes > passes).await;
}
async fn close(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn opt_in_wakes_the_existing_owner_and_retains_exact_utf16_rows_without_clients() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/task-occurrences.json")).unwrap();
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, fixture[0]["raw"].as_str().unwrap());
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    settled(&vault).await;
    assert!(
        call(
            &vault,
            DatabaseMethod::QueryProjection,
            vec![json!("tasks/occurrence")]
        )
        .await
        .is_err()
    );
    vault.set_task_occurrence_indexing(true).await.unwrap();
    until(async || {
        call(
            &vault,
            DatabaseMethod::QueryProjection,
            vec![json!("tasks/occurrence")],
        )
        .await
        .ok()
        .is_some_and(|result| {
            result["rows"]
                .as_array()
                .is_some_and(|rows| rows.len() == 1)
        })
    })
    .await;
    assert_eq!(vault.status().unwrap().clients, 0);
    assert_eq!(
        javascript_numbers(query(&vault).await["rows"].clone()),
        javascript_numbers(fixture[0]["expected"].clone()),
    );
    close(runtime).await;
}

#[tokio::test]
async fn edited_noncanonical_and_deleted_sources_converge_without_clients() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, &source("read"));
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.set_task_occurrence_indexing(true).await.unwrap();
    settled(&vault).await;
    assert_eq!(query(&vault).await["rows"][0]["outcome"], "pending");
    write(&root, &source("read").replace("[ ]", "[x]"));
    until(async || query(&vault).await["rows"][0]["outcome"] == "completed").await;
    write(
        &root,
        &source("read").replace("type: daily-note", "type: note"),
    );
    until(async || query(&vault).await["rows"] == json!([])).await;
    write(&root, &source("read"));
    until(async || query(&vault).await["rows"].as_array().unwrap().len() == 1).await;
    std::fs::remove_file(root.path().join(PATH)).unwrap();
    until(async || query(&vault).await["rows"] == json!([])).await;
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
}

#[tokio::test]
async fn cold_start_repairs_missing_sources_despite_unchanged_metadata_and_skips_noops() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, &source("read"));
    let id = Uuid::new_v4();
    let first = runtime(&profile);
    let old = first.admit(id, root.path()).unwrap();
    old.start_markdown_indexing().await.unwrap();
    settled(&old).await;
    close(first).await;
    let next = runtime(&profile);
    let vault = next.admit(id, root.path()).unwrap();
    vault.set_task_occurrence_indexing(true).await.unwrap();
    settled(&vault).await;
    assert_eq!(vault.indexing_status().unwrap().indexed, 0);
    assert_eq!(query(&vault).await["rows"].as_array().unwrap().len(), 1);
    let revision = query(&vault).await["revision"].clone();
    rescan(&vault, &root).await;
    assert_eq!(query(&vault).await["revision"], revision);
    close(next).await;
    let final_runtime = runtime(&profile);
    let reopened = final_runtime.admit(id, root.path()).unwrap();
    reopened.set_task_occurrence_indexing(true).await.unwrap();
    settled(&reopened).await;
    assert_eq!(query(&reopened).await["revision"], revision);
    close(final_runtime).await;
}

#[tokio::test]
async fn duplicate_and_oversized_sources_publish_stable_errors_without_partial_rows() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, &source("read"));
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.set_task_occurrence_indexing(true).await.unwrap();
    settled(&vault).await;
    let item = source("read").strip_prefix(HEADER).unwrap().to_owned();
    write(&root, &format!("{HEADER}{item}{item}"));
    until(async || query(&vault).await["indexStatus"] == "error").await;
    assert_eq!(query(&vault).await["rows"], json!([]));
    assert!(vault.indexing_status().unwrap().error.is_some());
    let mut too_many = HEADER.to_owned();
    for index in 0..65 {
        too_many.push_str(
            source(&format!("task-{index}"))
                .strip_prefix(HEADER)
                .unwrap(),
        );
    }
    write(&root, &too_many);
    until(async || {
        vault
            .indexing_status()
            .unwrap()
            .error
            .as_deref()
            .is_some_and(|error| error.contains("64"))
    })
    .await;
    assert_eq!(query(&vault).await["rows"], json!([]));
    let revision = query(&vault).await["revision"].clone();
    rescan(&vault, &root).await;
    assert_eq!(query(&vault).await["revision"], revision);
    write(&root, &source("repaired"));
    until(async || query(&vault).await["indexStatus"] == "ready").await;
    assert_eq!(query(&vault).await["rows"][0]["taskId"], "repaired");
    close(runtime).await;
}

#[tokio::test]
async fn disabling_tasks_keeps_markdown_running_and_reenable_repairs_the_source() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, &source("read"));
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.set_task_occurrence_indexing(true).await.unwrap();
    settled(&vault).await;
    vault.set_task_occurrence_indexing(false).await.unwrap();
    write(&root, &source("read").replace("[ ]", "[x]"));
    until(async || vault.indexing_status().unwrap().indexed >= 2).await;
    assert_eq!(query(&vault).await["rows"][0]["outcome"], "pending");
    vault.set_task_occurrence_indexing(true).await.unwrap();
    until(async || query(&vault).await["rows"][0]["outcome"] == "completed").await;
    close(runtime).await;
}
