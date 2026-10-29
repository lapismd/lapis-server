use super::*;
#[tokio::test]
async fn native_tasks_port_uses_shared_dirty_and_version_guards_without_fallback() {
    let f = Fixture::new("complete / schedule / count");
    let runtime = f.runtime();
    let (vault, client) = configured(&f, &runtime).await;
    let initial = snapshot(&vault);
    let mutation = NativeTaskMutation::Cancel {
        at: "2026-08-20T10:00:00Z".into(),
    };
    let editor = client.open_editor("tasks/Read.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    let rejected = client
        .control_tasks(NativeTaskCommand::Mutate {
            path: initial.path.clone(),
            expected_version: initial.version.clone(),
            mutation: mutation.clone(),
        })
        .await
        .unwrap();
    assert!(
        matches!(rejected,NativeTaskValue::Failure{ref failure} if failure.code.as_deref()==Some("BUSY") && !failure.uncertain)
    );
    assert_eq!(snapshot(&vault).text, initial.text);
    client.discard_editor(editor).unwrap();
    let changed = client
        .control_tasks(NativeTaskCommand::Mutate {
            path: initial.path.clone(),
            expected_version: initial.version.clone(),
            mutation: mutation.clone(),
        })
        .await
        .unwrap();
    assert!(matches!(changed, NativeTaskValue::Snapshot { .. }));
    let stale = client
        .control_tasks(NativeTaskCommand::Mutate {
            path: initial.path.clone(),
            expected_version: initial.version,
            mutation,
        })
        .await
        .unwrap();
    assert!(
        matches!(stale,NativeTaskValue::Failure{ref failure} if failure.code.as_deref()==Some("CONFLICT"))
    );
    client
        .control_tasks(NativeTaskCommand::Disable {})
        .await
        .unwrap();
    assert_eq!(
        client
            .control_tasks(NativeTaskCommand::Retry {
                request_id: Uuid::new_v4()
            })
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Unsupported
    );
    close(runtime).await;
}

#[tokio::test]
async fn native_tasks_port_provider_failure_is_explicit_and_does_not_advance_task() {
    let f = Fixture::new("complete / schedule / count");
    let runtime = f.runtime();
    let (vault, client) = configured(&f, &runtime).await;
    let before = snapshot(&vault);
    let request = f.request(&vault);
    let mut configuration = f.configuration();
    configuration.provider = NativeTaskProvider::Unavailable {
        message: "Ambiguous provider".into(),
    };
    client
        .control_tasks(NativeTaskCommand::Configure { configuration })
        .await
        .unwrap();
    let failed = client
        .control_tasks(NativeTaskCommand::Submit {
            request: request.clone(),
        })
        .await
        .unwrap();
    assert!(
        matches!(failed,NativeTaskValue::Failure{ref failure} if failure.request_id==Some(request.request_id) && failure.code.as_deref()==Some("DAILY_PROVIDER_UNAVAILABLE") && !failure.uncertain)
    );
    assert_eq!(snapshot(&vault).text, before.text);
    let inspected = client
        .control_tasks(NativeTaskCommand::Inspect {
            request_id: request.request_id,
        })
        .await
        .unwrap();
    assert!(matches!(
        inspected,
        NativeTaskValue::Operation { operation: None }
    ));
    close(runtime).await;
}

#[tokio::test]
async fn native_tasks_port_rejects_unknown_nested_fields_and_hidden_source_admission() {
    let f = Fixture::new("complete / schedule / count");
    let runtime = f.runtime();
    let (vault, client) = configured(&f, &runtime).await;
    assert!(serde_json::from_value::<NativeTaskCommand>(json!({"kind":"mutate","path":"tasks/Read.md","expected_version":snapshot(&vault).version,"mutation":{"type":"cancel","at":"today","callback":"write"}})).is_err());
    assert!(serde_json::from_value::<NativeTaskCommand>(json!({"kind":"mutate","path":"tasks/Read.md","expected_version":snapshot(&vault).version,"mutation":{"type":"complete","at":"today","localDate":"2026-08-20"}})).is_err());
    for path in [
        ".lapis/Task.md",
        "../Task.md",
        "/outside/Task.md",
        "Task.txt",
    ] {
        assert_eq!(
            client
                .control_tasks(NativeTaskCommand::Mutate {
                    path: path.into(),
                    expected_version: snapshot(&vault).version,
                    mutation: NativeTaskMutation::Cancel { at: "today".into() }
                })
                .await
                .unwrap_err()
                .code(),
            ErrorCode::InvalidRequest
        );
    }
    let peer = vault.attach().unwrap();
    drop(client);
    assert!(
        peer.control_tasks(NativeTaskCommand::Status {})
            .await
            .is_ok()
    );
    close(runtime).await;
}

#[test]
fn native_tasks_storage_rejects_vault_content_and_replaced_physical_roots() {
    let f = Fixture::new("complete / schedule / count");
    let local = f.root.join(".lapis");
    fs::create_dir(&local).unwrap();
    let runtime = Runtime::default().with_tasks(TaskStorage::new(&local).unwrap());
    assert!(
        matches!(runtime.admit(f.id, &f.root), Err(error) if error.code() == ErrorCode::PermissionDenied)
    );
    let runtime = f.runtime();
    fs::rename(f.profile.join("tasks"), f.profile.join("old")).unwrap();
    fs::create_dir(f.profile.join("tasks")).unwrap();
    assert!(runtime.admit(f.id, &f.root).is_err());
}
