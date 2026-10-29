use lapis_app_core::{
    embedding::host::NativeEmbeddingHost,
    services::{Client, DatabaseStorage, Runtime, VaultService},
};
use lapis_app_protocol::{
    ErrorCode,
    database::call::{DatabaseCall, DatabaseMethod, DatabaseValue},
    indexing::{IndexingCommand, IndexingValue},
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

fn runtime(profile: &tempfile::TempDir) -> Arc<Runtime> {
    Arc::new(Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    ))
}
async fn close(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
async fn command(client: &Client, value: Value) -> IndexingValue {
    client
        .control_indexing(serde_json::from_value(value).expect("Search control contract"))
        .await
        .unwrap()
}
fn selection(providers: &[&str]) -> Value {
    json!({"kind":"select_search", "selection": {
        "providers": providers.iter().map(|id| json!({"provider_id":id,"version":"1","priority":0})).collect::<Vec<_>>(),
        "chunking":{"targetChars":256,"breakpointWindowChars":32,"breakpointDecay":0.5}
    }})
}
async fn read(vault: &Arc<VaultService>, path: &str) -> Option<Value> {
    match vault
        .attach()
        .unwrap()
        .call_database(DatabaseCall {
            method: DatabaseMethod::GetSearchDocument,
            args: vec![DatabaseValue::Value { value: json!(path) }],
        })
        .await
        .unwrap()
    {
        DatabaseValue::Undefined {} => None,
        DatabaseValue::Value { value } if value.is_null() => None,
        DatabaseValue::Value { value } => Some(value),
    }
}
async fn until(mut ready: impl AsyncFnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !ready().await {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Search control must converge");
}
async fn refresh(client: &Client) {
    let value = command(client, json!({"kind":"refresh","rebuild":false})).await;
    assert!(value.status.unwrap().error.is_none());
}

#[tokio::test]
async fn compiled_registry_controls_continue_search_after_client_detach() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.markdown"), "# First\n").unwrap();
    std::fs::write(
        root.path().join("Diagram.canvas"),
        r#"{"nodes":[{"text":"pear"}]}"#,
    )
    .unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    let selected = command(&client, selection(&["search:markdown", "search:canvas"])).await;
    assert_eq!(
        serde_json::to_value(selected.contribution.unwrap()).unwrap()["placement"],
        "native"
    );
    let status = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let status = vault.indexing_status().unwrap();
            if !status.active {
                break serde_json::to_value(status.progress).unwrap();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("native pass must settle before checking completed progress");
    assert_eq!(status["processed"], 2, "Search progress includes Canvas");
    assert_eq!(status["total"], 2);
    assert!(
        status["completed_at"].is_number(),
        "native completion time is observable"
    );
    refresh(&client).await;
    assert_eq!(
        read(&vault, "Diagram.canvas").await.unwrap()["content"],
        "pear"
    );
    assert_eq!(
        read(&vault, "Note.markdown").await.unwrap()["sourceMetadata"]["chunking"]["targetChars"],
        256.0
    );
    drop(client);
    assert_eq!(vault.status().unwrap().clients, 0);
    std::fs::write(
        root.path().join("Diagram.canvas"),
        r#"{"nodes":[{"text":"plum"}]}"#,
    )
    .unwrap();
    until(async || {
        read(&vault, "Diagram.canvas")
            .await
            .is_some_and(|row| row["content"] == "plum")
    })
    .await;
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
}

#[tokio::test]
async fn failed_refresh_retains_the_last_successful_completion_time() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.md"), "# Retained\n").unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    command(&client, selection(&["search:markdown"])).await;
    refresh(&client).await;
    std::fs::create_dir_all(root.path().join(".lapis")).unwrap();
    std::fs::write(root.path().join(".lapis/types.json"), "invalid JSON").unwrap();
    let failed = command(&client, json!({"kind":"refresh","rebuild":false}))
        .await
        .status
        .unwrap();
    assert!(failed.error.is_some());
    assert!(failed.completed_at.is_some());
    let retried = command(&client, json!({"kind":"refresh","rebuild":true}))
        .await
        .status
        .unwrap();
    assert!(retried.error.is_some());
    assert_eq!(retried.completed_at, failed.completed_at);
    close(runtime).await;
}

#[tokio::test]
async fn native_selection_fences_generated_writes_and_disable_drains_before_releasing_them() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.md"), "# Native\n").unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    command(&client, selection(&["search:markdown"])).await;
    refresh(&client).await;
    for (method, args) in [
        (DatabaseMethod::UpsertSearchDocument, vec![]),
        (DatabaseMethod::DeleteSearchDocument, vec![json!("Note.md")]),
        (
            DatabaseMethod::SetMeta,
            vec![
                json!("search.reconcile-checkpoint.v1"),
                json!({"forged":true}),
            ],
        ),
    ] {
        let error = client
            .call_database(DatabaseCall {
                method,
                args: args
                    .into_iter()
                    .map(|value| DatabaseValue::Value { value })
                    .collect(),
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::PermissionDenied, "{method:?}");
    }
    let native = read(&vault, "Note.md").await.unwrap();
    command(&client, json!({"kind":"disable_search"})).await;
    client
        .call_database(DatabaseCall {
            method: DatabaseMethod::SetMeta,
            args: vec![
                DatabaseValue::Value {
                    value: json!("search.reconcile-checkpoint.v1"),
                },
                DatabaseValue::Value {
                    value: json!({"client":"released"}),
                },
            ],
        })
        .await
        .unwrap();
    client
        .call_database(DatabaseCall {
            method: DatabaseMethod::DeleteSearchDocument,
            args: vec![DatabaseValue::Value {
                value: json!("Note.md"),
            }],
        })
        .await
        .unwrap();
    refresh(&client).await;
    assert!(read(&vault, "Note.md").await.is_none());
    assert_eq!(native["content"], "# Native\n");
    close(runtime).await;
}

#[tokio::test]
async fn unsupported_registry_is_visible_and_stops_native_search_without_a_fallback() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.md"), "# Before\n").unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    command(&client, selection(&["search:markdown"])).await;
    refresh(&client).await;
    let value = command(&client, selection(&["search:markdown", "community:custom"])).await;
    let contribution = serde_json::to_value(value.contribution.unwrap()).unwrap();
    assert_eq!(contribution["placement"], "client_dependent");
    assert!(
        contribution["message"]
            .as_str()
            .unwrap()
            .contains("community:custom")
    );
    std::fs::write(root.path().join("Note.md"), "# After!\n").unwrap();
    refresh(&client).await;
    assert_eq!(
        read(&vault, "Note.md").await.unwrap()["content"],
        "# Before\n"
    );
    let error = client
        .call_database(DatabaseCall {
            method: DatabaseMethod::DeleteSearchDocument,
            args: vec![],
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::PermissionDenied);
    command(&client, selection(&["search:markdown"])).await;
    refresh(&client).await;
    assert_eq!(
        read(&vault, "Note.md").await.unwrap()["content"],
        "# After!\n"
    );
    close(runtime).await;
}

#[tokio::test]
async fn replacing_registry_prunes_removed_compiled_sources_and_retains_foreign_rows() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.md"), "# Retained\n").unwrap();
    std::fs::write(
        root.path().join("Diagram.canvas"),
        r#"{"nodes":[{"text":"pear"}]}"#,
    )
    .unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    client.call_database(DatabaseCall {method:DatabaseMethod::UpsertSearchDocument,args:vec![DatabaseValue::Value {value:json!({
        "path":"Absent.custom","sourceProviderId":"independent:source","name":"Absent.custom","extension":"custom","checksum":"retained","content":"foreign source","tags":[],"tagParts":[],"tagHierarchy":[]
    })}]}).await.unwrap();
    command(&client, selection(&["search:markdown", "search:canvas"])).await;
    refresh(&client).await;
    assert!(read(&vault, "Diagram.canvas").await.is_some());
    command(&client, selection(&["search:markdown"])).await;
    refresh(&client).await;
    assert!(read(&vault, "Diagram.canvas").await.is_none());
    assert!(read(&vault, "Note.md").await.is_some());
    command(&client, selection(&[])).await;
    refresh(&client).await;
    assert!(read(&vault, "Note.md").await.is_none());
    assert_eq!(
        read(&vault, "Absent.custom").await.unwrap()["content"],
        "foreign source"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("Note.md")).unwrap(),
        "# Retained\n"
    );
    close(runtime).await;
}

#[tokio::test]
async fn invalid_registry_fails_before_selection_and_version_mismatch_requires_a_client() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    let duplicate: IndexingCommand =
        serde_json::from_value(selection(&["search:markdown", "search:markdown"])).unwrap();
    assert_eq!(
        client.control_indexing(duplicate).await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        vault.indexing_status().unwrap_err().code(),
        ErrorCode::Unsupported
    );
    let mut invalid = selection(&["search:markdown"]);
    invalid["selection"]["chunking"]["targetChars"] = json!(0);
    assert_eq!(
        client
            .control_indexing(serde_json::from_value(invalid).unwrap())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        vault.indexing_status().unwrap_err().code(),
        ErrorCode::Unsupported
    );
    let mut unsupported = selection(&["search:markdown"]);
    unsupported["selection"]["providers"][0]["version"] = json!("next");
    let result = command(&client, unsupported).await;
    assert_eq!(
        serde_json::to_value(result.contribution.unwrap()).unwrap()["placement"],
        "client_dependent"
    );
    close(runtime).await;
}
