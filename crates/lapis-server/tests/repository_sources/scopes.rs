use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn managed_and_original_read_scopes_are_distinct_client_bound_and_independently_retired() {
    let f = Fixture::with_database().await;
    let root = f.roots[0].path();
    fs::write(root.join("Note.md"), "saved original bytes").unwrap();
    init(root);
    let before = metadata(root);
    let client = f.attach(f.ids[0]).await;
    let managed = json!(f.ids[0]);
    let original = sources(&f, client.client_id, "open_original").await;
    assert!(original["token"].as_str().unwrap().starts_with("original:"));
    assert_eq!(
        sources(&f, client.client_id, "open_original").await,
        original
    );
    let token = original["token"].clone();
    let original_heads = read(&f, client.client_id, "heads", json!({"vault":token})).await;
    let managed_heads = read(&f, client.client_id, "heads", json!({"vault":managed})).await;
    assert_ne!(original_heads["repository"], managed_heads["repository"]);
    assert!(!original_heads["heads"].as_array().unwrap().is_empty());
    let original_handle = prepared(&f, client.client_id, &token).await;
    let managed_handle = prepared(&f, client.client_id, &managed).await;
    for (scope, handle) in [(&token, &original_handle), (&managed, &managed_handle)] {
        let result = read(
            &f,
            client.client_id,
            "read",
            json!({"vault":scope,"handle":handle,"query":{"kind":"query_info"}}),
        )
        .await;
        assert_eq!(result["kind"], "query_info");
    }
    assert!(
        !history(
            &f,
            client.client_id,
            "status",
            json!({"vault":managed,"handle":original_handle})
        )
        .await
        .status()
        .is_success()
    );
    let peer = f.attach(f.ids[0]).await;
    assert_eq!(
        history(&f, peer.client_id, "heads", json!({"vault":token}))
            .await
            .status(),
        403
    );
    let peer_token = sources(&f, peer.client_id, "open_original").await["token"].clone();
    assert_ne!(peer_token, token);
    let peer_heads = read(&f, peer.client_id, "heads", json!({"vault":peer_token})).await;
    assert_eq!(peer_heads["repository"], original_heads["repository"]);
    assert_eq!(peer_heads["generation"], original_heads["generation"]);
    f.detach(&client).await;
    assert_eq!(
        history(&f, client.client_id, "heads", json!({"vault":token}))
            .await
            .status(),
        403
    );
    assert!(
        !read(&f, peer.client_id, "heads", json!({"vault":peer_token})).await["heads"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(metadata(root), before);
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "saved original bytes"
    );
    f.detach(&peer).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn original_reads_work_when_capture_is_paused_or_managed_startup_failed_without_fallback() {
    for broken in [false, true] {
        let f = Fixture::with_database().await;
        let root = f.roots[0].path();
        fs::write(root.join("Note.md"), "independent original bytes").unwrap();
        init(root);
        let history_path = f
            .profile
            .as_ref()
            .unwrap()
            .path()
            .join("vault-history")
            .join(f.ids[0].to_string());
        if broken {
            fs::create_dir_all(&history_path).unwrap();
            fs::write(history_path.join("repo"), "broken managed source").unwrap();
        } else {
            fs::create_dir(root.join(".lapis")).unwrap();
            fs::write(
                root.join(".lapis/app.json"),
                r#"{"pluginData":{"repository-history":{"automaticBackup":false}}}"#,
            )
            .unwrap();
        }
        let before = metadata(root);
        let client = f.attach(f.ids[0]).await;
        let original = sources(&f, client.client_id, "open_original").await;
        let handle = prepared(&f, client.client_id, &original["token"]).await;
        assert_eq!(
            read(
                &f,
                client.client_id,
                "read",
                json!({"vault":original["token"],"handle":handle,"query":{"kind":"query_info"}})
            )
            .await["kind"],
            "query_info"
        );
        let managed = history(&f, client.client_id, "heads", json!({"vault":f.ids[0]})).await;
        if broken {
            assert!(!managed.status().is_success());
            assert_eq!(
                fs::read_to_string(history_path.join("repo")).unwrap(),
                "broken managed source"
            );
        } else {
            assert_eq!(
                managed.json::<Value>().await.unwrap()["data"]["heads"],
                json!([])
            );
            assert!(!history_path.exists());
        }
        assert_eq!(metadata(root), before);
        f.detach(&client).await;
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replaced_original_metadata_fences_old_scopes_and_rebinds_only_a_fresh_client_epoch() {
    let f = Fixture::with_database().await;
    let root = f.roots[0].path();
    init(root);
    let client = f.attach(f.ids[0]).await;
    let token = sources(&f, client.client_id, "open_original").await["token"].clone();
    let before = read(&f, client.client_id, "heads", json!({"vault":token})).await;
    let preserved = tempfile::tempdir().unwrap();
    let away = preserved.path().join("repo");
    fs::rename(root.join(".jj/repo"), &away).unwrap();
    copy_tree(&away, &root.join(".jj/repo"));
    assert_eq!(
        history(&f, client.client_id, "heads", json!({"vault":token}))
            .await
            .status(),
        409
    );
    assert!(
        !source_response(&f, client.client_id, json!({"kind":"open_original"}))
            .await
            .status()
            .is_success()
    );
    f.detach(&client).await;
    let fresh = f.attach(f.ids[0]).await;
    let token = sources(&f, fresh.client_id, "open_original").await["token"].clone();
    let after = read(&f, fresh.client_id, "heads", json!({"vault":token})).await;
    assert_eq!(after["repository"], before["repository"]);
    assert_ne!(after["generation"], before["generation"]);
    f.detach(&fresh).await;
}
