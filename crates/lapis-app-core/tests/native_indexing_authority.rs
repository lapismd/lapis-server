use lapis_app_core::{
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime},
};
use lapis_app_protocol::{
    ErrorCode,
    database::call::{DatabaseCall, DatabaseMethod, DatabaseValue},
};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn native_metadata_selection_refuses_client_generated_writes() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.md"), "# Native note\n").unwrap();
    let runtime = Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    );
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    vault.start_markdown_indexing().await.unwrap();
    let result = client
        .call_database(DatabaseCall {
            method: DatabaseMethod::DeleteIndexedFile,
            args: vec![DatabaseValue::Value {
                value: json!("Note.md"),
            }],
        })
        .await;
    assert_eq!(
        result
            .expect_err("the native metadata owner must reject a client writer")
            .code(),
        ErrorCode::PermissionDenied
    );
    drop(client);
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

fn database_call(method: DatabaseMethod, args: Vec<serde_json::Value>) -> DatabaseCall {
    DatabaseCall {
        method,
        args: args
            .into_iter()
            .map(|value| DatabaseValue::Value { value })
            .collect(),
    }
}
async fn value(
    client: &lapis_app_core::services::Client,
    method: DatabaseMethod,
    args: Vec<serde_json::Value>,
) -> serde_json::Value {
    match client
        .call_database(database_call(method, args))
        .await
        .unwrap()
    {
        DatabaseValue::Value { value } => value,
        DatabaseValue::Undefined {} => serde_json::Value::Null,
    }
}
async fn close(runtime: Runtime) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn typed_projection_selection_preserves_queries_detach_and_explicit_disable() {
    use lapis_app_protocol::indexing::*;
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("docs")).unwrap();
    std::fs::write(
        root.path().join("docs/Note.doc.md"),
        "---\nid: first\ntitle: First\n---\n# Body\n",
    )
    .unwrap();
    let runtime = Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    );
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    assert!(
        client
            .control_indexing(IndexingCommand::Status {})
            .await
            .unwrap()
            .status
            .is_none()
    );
    let selected = client
        .control_indexing(IndexingCommand::RegisterProjection {
            contribution: ProjectionContribution {
                projection_id: "docs/note".into(),
                owner_plugin_id: "docs".into(),
                schema_version: 1.0,
                configuration_hash: String::new(),
                path_prefix: Some("docs".into()),
            },
        })
        .await
        .unwrap();
    assert!(matches!(
        selected.contribution.unwrap().placement,
        ContributionPlacement::Native
    ));
    let refreshed = client
        .control_indexing(IndexingCommand::Refresh { rebuild: false })
        .await
        .unwrap()
        .status
        .unwrap();
    assert!(refreshed.error.is_none(), "{:?}", refreshed.error);
    assert!(!refreshed.active);
    assert_eq!((refreshed.processed, refreshed.total), (1, 1));
    assert_eq!(
        value(
            &client,
            DatabaseMethod::GetProjectionRow,
            vec![json!("docs/note"), json!("docs/Note.doc.md")]
        )
        .await["title"],
        "First"
    );
    assert_eq!(
        client
            .call_database(database_call(
                DatabaseMethod::UnregisterProjectionDefinition,
                vec![json!("docs/note")]
            ))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PermissionDenied
    );
    let before = vault.indexing_status().unwrap().passes;
    drop(client);
    std::fs::write(
        root.path().join("docs/Note.doc.md"),
        "---\nid: first\ntitle: Second\n---\n# Body\n",
    )
    .unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let work = vault.begin_work().unwrap();
        let row = work
            .call_database(database_call(
                DatabaseMethod::GetProjectionRow,
                vec![json!("docs/note"), json!("docs/Note.doc.md")],
            ))
            .await
            .unwrap();
        if matches!(row, DatabaseValue::Value { value } if value["title"] == "Second") {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(vault.status().unwrap().clients, 0);
    assert!(vault.indexing_status().unwrap().passes > before);
    let reattached = vault.attach().unwrap();
    reattached
        .control_indexing(IndexingCommand::DisableProjection {
            projection_id: "docs/note".into(),
        })
        .await
        .unwrap();
    reattached
        .control_indexing(IndexingCommand::Refresh { rebuild: false })
        .await
        .unwrap();
    assert!(
        reattached
            .call_database(database_call(
                DatabaseMethod::QueryProjection,
                vec![json!("docs/note")]
            ))
            .await
            .is_err()
    );
    reattached
        .call_database(database_call(
            DatabaseMethod::UnregisterProjectionDefinition,
            vec![json!("docs/note")],
        ))
        .await
        .unwrap();
    drop(reattached);
    close(runtime).await;
}

#[tokio::test]
async fn custom_processors_are_observable_without_selecting_a_fallback_writer() {
    use lapis_app_protocol::indexing::*;
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    );
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    let response = client
        .control_indexing(IndexingCommand::RegisterProcessor {
            owner_plugin_id: "custom".into(),
            extension: "canvas".into(),
        })
        .await
        .unwrap();
    assert!(response.status.is_none());
    let unavailable = response.contribution.unwrap();
    assert!(matches!(
        unavailable.placement,
        ContributionPlacement::ClientDependent
    ));
    assert!(unavailable.message.unwrap().contains("requires a client"));
    for command in [
        IndexingCommand::RegisterProcessor {
            owner_plugin_id: "x".repeat(129),
            extension: "md".into(),
        },
        IndexingCommand::DisableProjection {
            projection_id: "x".repeat(129),
        },
        IndexingCommand::RegisterProjection {
            contribution: ProjectionContribution {
                projection_id: "docs/note".into(),
                owner_plugin_id: "docs".into(),
                schema_version: 1.0,
                configuration_hash: String::new(),
                path_prefix: Some("x".repeat(1025)),
            },
        },
    ] {
        assert_eq!(
            client.control_indexing(command).await.unwrap_err().code(),
            ErrorCode::InvalidRequest
        );
    }
    let response = client
        .control_indexing(IndexingCommand::RegisterProcessor {
            owner_plugin_id: "markdown".into(),
            extension: "md".into(),
        })
        .await
        .unwrap();
    assert!(response.status.is_some());
    assert!(matches!(
        response.contribution.unwrap().placement,
        ContributionPlacement::Native
    ));
    client
        .call_database(database_call(
            DatabaseMethod::SetMeta,
            vec![json!("configuration"), json!(true)],
        ))
        .await
        .unwrap();
    assert_eq!(
        value(
            &client,
            DatabaseMethod::GetMeta,
            vec![json!("configuration")]
        )
        .await,
        true
    );
    assert_eq!(
        client
            .call_database(database_call(
                DatabaseMethod::SetMeta,
                vec![json!("metadata-cache.reconcile-checkpoint.v1"), json!({})]
            ))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PermissionDenied
    );
    drop(client);
    close(runtime).await;
}

#[tokio::test]
async fn native_rebuild_repairs_generated_payload_without_a_client_delete() {
    use lapis_app_protocol::indexing::IndexingCommand;
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.md"), "# Original\n").unwrap();
    let runtime = Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    );
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    client
        .control_indexing(IndexingCommand::SelectMetadata {})
        .await
        .unwrap();
    client
        .control_indexing(IndexingCommand::Refresh { rebuild: false })
        .await
        .unwrap();
    let mut row = value(
        &client,
        DatabaseMethod::GetIndexedFile,
        vec![json!("Note.md")],
    )
    .await;
    row["metadata"]["metadata"]["headings"] = json!([]);
    let work = vault.begin_work().unwrap();
    work.call_database(database_call(DatabaseMethod::UpsertIndexedFile, vec![row]))
        .await
        .unwrap();
    drop(work);
    client
        .control_indexing(IndexingCommand::Refresh { rebuild: true })
        .await
        .unwrap();
    assert_eq!(
        value(
            &client,
            DatabaseMethod::GetIndexedFile,
            vec![json!("Note.md")]
        )
        .await["metadata"]["metadata"]["headings"][0]["heading"],
        "Original"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("Note.md")).unwrap(),
        "# Original\n"
    );
    drop(client);
    close(runtime).await;
}

#[tokio::test]
async fn typed_tasks_declarations_select_compiled_algorithms_and_fence_both_namespaces() {
    use lapis_app_protocol::indexing::*;
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("Task.md"),
        "---\ntype: task\nid: task-one\n---\n# Task\n",
    )
    .unwrap();
    let runtime = Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    );
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    for (id, version) in [
        (
            "tasks/task",
            f64::from(lapis_task_services::TASK_PROJECTION_VERSION),
        ),
        (
            "tasks/occurrence",
            f64::from(lapis_task_services::TASK_OCCURRENCE_PROJECTION_VERSION),
        ),
    ] {
        let response = client
            .control_indexing(IndexingCommand::RegisterProjection {
                contribution: ProjectionContribution {
                    projection_id: id.into(),
                    owner_plugin_id: "tasks".into(),
                    schema_version: version,
                    configuration_hash: String::new(),
                    path_prefix: None,
                },
            })
            .await
            .unwrap();
        assert!(matches!(
            response.contribution.unwrap().placement,
            ContributionPlacement::Native
        ));
        assert_eq!(
            client
                .call_database(database_call(
                    DatabaseMethod::UnregisterProjectionDefinition,
                    vec![json!(id)]
                ))
                .await
                .unwrap_err()
                .code(),
            ErrorCode::PermissionDenied
        );
    }
    let status = client
        .control_indexing(IndexingCommand::Refresh { rebuild: false })
        .await
        .unwrap()
        .status
        .unwrap();
    assert!(status.error.is_none(), "{:?}", status.error);
    assert_eq!(
        value(&client, DatabaseMethod::QueryTasks, vec![json!({})])
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        client
            .call_database(database_call(
                DatabaseMethod::DeleteTaskProjection,
                vec![json!("Task.md")]
            ))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PermissionDenied
    );
    for id in ["tasks/task", "tasks/occurrence"] {
        client
            .control_indexing(IndexingCommand::DisableProjection {
                projection_id: id.into(),
            })
            .await
            .unwrap();
        client
            .call_database(database_call(
                DatabaseMethod::UnregisterProjectionDefinition,
                vec![json!(id)],
            ))
            .await
            .unwrap();
    }
    drop(client);
    close(runtime).await;
}
