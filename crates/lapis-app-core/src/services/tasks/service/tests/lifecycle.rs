use super::*;
#[tokio::test]
async fn native_tasks_port_preserves_retained_bytes_and_cold_idempotent_identity() {
    let f = Fixture::new("complete / schedule / count");
    let runtime = f.runtime();
    let (vault, client) = configured(&f, &runtime).await;
    let request = f.request(&vault);
    let NativeTaskValue::Operation {
        operation: Some(result),
    } = client
        .control_tasks(NativeTaskCommand::Submit {
            request: request.clone(),
        })
        .await
        .unwrap()
    else {
        panic!("submit")
    };
    assert!(matches!(
        result.disposition,
        NativeTaskDisposition::Completed
    ));
    assert_eq!(
        result.task_replacement,
        f.case["expected"]["attempts"][0]["taskRaw"]
    );
    assert_eq!(snapshot(&vault).text, result.task_replacement);
    let daily = result.daily.as_ref().unwrap();
    assert_eq!(
        fs::read_to_string(f.root.join(&daily.path)).unwrap(),
        daily.text
    );
    let mut unavailable = f.configuration();
    unavailable.provider = NativeTaskProvider::Unavailable {
        message: "Custom provider selected".into(),
    };
    client
        .control_tasks(NativeTaskCommand::Configure {
            configuration: unavailable,
        })
        .await
        .unwrap();
    // Existing identity must win over stale source and now-unavailable provider.
    let duplicate = client
        .control_tasks(NativeTaskCommand::Submit {
            request: request.clone(),
        })
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(duplicate).unwrap(),
        serde_json::to_value(NativeTaskValue::Operation {
            operation: Some(result)
        })
        .unwrap()
    );
    drop(client);
    close(runtime).await;
    let reopened = f.runtime();
    let vault = reopened.admit(f.id, &f.root).unwrap();
    let peer = vault.attach().unwrap();
    let before = snapshot(&vault);
    let inspected = peer
        .control_tasks(NativeTaskCommand::Inspect {
            request_id: request.request_id,
        })
        .await
        .unwrap();
    assert!(
        matches!(inspected,NativeTaskValue::Operation{operation:Some(ref operation)} if matches!(operation.disposition,NativeTaskDisposition::Completed))
    );
    assert_eq!(snapshot(&vault).text, before.text);
    close(reopened).await;
}

#[tokio::test]
async fn native_tasks_port_retains_partial_daily_failure_for_read_only_inspection_and_explicit_retry()
 {
    let f = Fixture::new("complete / schedule / count");
    let runtime = f.runtime();
    let (vault, client) = configured(&f, &runtime).await;
    let before = snapshot(&vault);
    let request = f.request(&vault);
    let daily = "daily/2026-08-20.md";
    let editor = client.open_editor(daily).unwrap();
    client.mark_dirty(editor, 1).unwrap();
    let failed = client
        .control_tasks(NativeTaskCommand::Submit {
            request: request.clone(),
        })
        .await
        .unwrap();
    let NativeTaskValue::Failure { failure } = failed else {
        panic!("dirty daily must refuse")
    };
    assert_eq!(failure.request_id, Some(request.request_id));
    assert_eq!(failure.code.as_deref(), Some("BUSY"));
    assert!(!failure.uncertain);
    assert!(failure.operation.is_some());
    assert_eq!(snapshot(&vault).text, before.text);
    let daily_before = fs::read_to_string(f.root.join(daily)).unwrap();
    let inspected = client
        .control_tasks(NativeTaskCommand::Inspect {
            request_id: request.request_id,
        })
        .await
        .unwrap();
    assert!(
        matches!(inspected,NativeTaskValue::Operation{operation:Some(ref operation)} if matches!(operation.disposition,NativeTaskDisposition::Retryable))
    );
    assert_eq!(
        fs::read_to_string(f.root.join(daily)).unwrap(),
        daily_before
    );
    client.discard_editor(editor).unwrap();
    let result = client
        .control_tasks(NativeTaskCommand::Retry {
            request_id: request.request_id,
        })
        .await
        .unwrap();
    assert!(
        matches!(result,NativeTaskValue::Operation{operation:Some(ref operation)} if matches!(operation.disposition,NativeTaskDisposition::Completed))
    );
    assert_eq!(
        snapshot(&vault).text,
        f.case["expected"]["attempts"][0]["taskRaw"]
    );
    close(runtime).await;
}

#[tokio::test]
async fn native_tasks_port_derives_occurrence_identity_from_guarded_task_source() {
    let f = Fixture::new("complete / schedule / count");
    let runtime = f.runtime();
    let (vault, client) = configured(&f, &runtime).await;
    let original = snapshot(&vault);
    let result = client
        .control_tasks(NativeTaskCommand::Occurrence {
            path: original.path.clone(),
            expected_version: original.version.clone(),
            date: "2026-08-20".into(),
            completion: None,
        })
        .await
        .unwrap();
    let NativeTaskValue::Observation {
        document: saved,
        observation,
    } = result
    else {
        panic!("observation")
    };
    assert_eq!(observation.task_id, "read");
    assert_eq!(observation.task_path, original.path);
    assert_eq!(observation.occurrence_date, "2026-08-20");
    assert_eq!(
        saved.text,
        fs::read_to_string(f.root.join(&saved.path)).unwrap()
    );
    assert_eq!(snapshot(&vault).text, original.text);
    let failed = client
        .control_tasks(NativeTaskCommand::Occurrence {
            path: original.path,
            expected_version: "0".repeat(64),
            date: "2026-08-20".into(),
            completion: None,
        })
        .await
        .unwrap();
    assert!(
        matches!(failed,NativeTaskValue::Failure{ref failure} if failure.code.as_deref()==Some("CONFLICT"))
    );
    assert_eq!(
        saved.text,
        fs::read_to_string(f.root.join(&saved.path)).unwrap()
    );
    close(runtime).await;
}

#[tokio::test]
async fn retired_reader_client_cannot_readmit_native_tasks_through_retained_arc() {
    let f = Fixture::new("complete / schedule / count");
    let runtime = f.runtime();
    let (vault, client) = configured(&f, &runtime).await;
    let retained = Arc::new(client);
    let request = f.request(&vault);
    let before = snapshot(&vault);
    retained.detach().unwrap();
    retained.detach().unwrap();
    for command in [
        NativeTaskCommand::Status {},
        NativeTaskCommand::Submit {
            request: request.clone(),
        },
    ] {
        assert_eq!(
            retained.control_tasks(command).await.unwrap_err().code(),
            lapis_app_protocol::ErrorCode::PermissionDenied
        );
    }
    assert_eq!(snapshot(&vault).text, before.text);
    let peer = vault.attach().unwrap();
    assert!(
        matches!(peer.control_tasks(NativeTaskCommand::Status {}).await.unwrap(),
        NativeTaskValue::Status { status } if status.enabled)
    );
    assert!(matches!(
        peer.control_tasks(NativeTaskCommand::Inspect {
            request_id: request.request_id
        })
        .await
        .unwrap(),
        NativeTaskValue::Operation { operation: None }
    ));
    drop(retained);
    close(runtime).await;
}
