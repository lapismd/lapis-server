use super::*;

fn jj(root: &Path, args: &[&str]) {
    let result = Command::new("jj")
        .args([
            "--config",
            "user.name=\"Source fixture\"",
            "--config",
            "user.email=\"source@example.invalid\"",
        ])
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn internal_and_shared_original_layouts_preserve_external_heads_and_selected_pins() {
    for shared in [false, true] {
        let f = Fixture::with_database().await;
        let root = f.roots[0].path();
        let external = tempfile::tempdir().unwrap();
        if shared {
            jj(external.path(), &["git", "init"]);
            let linked = external.path().join("linked");
            jj(
                external.path(),
                &["workspace", "add", linked.to_str().unwrap()],
            );
            fs::rename(linked.join(".jj"), root.join(".jj")).unwrap();
            fs::write(
                root.join(".jj/repo"),
                external
                    .path()
                    .join(".jj/repo")
                    .as_os_str()
                    .as_encoded_bytes(),
            )
            .unwrap();
        } else {
            jj(root, &["git", "init"]);
        }
        let before = metadata(root);
        let external_before = metadata(external.path());
        let client = f.attach(f.ids[0]).await;
        let token = sources(&f, client.client_id, "open_original").await["token"].clone();
        let heads = read(&f, client.client_id, "heads", json!({"vault":token})).await;
        let handle = prepared(&f, client.client_id, &token).await;
        assert_eq!(metadata(root), before);
        assert_eq!(metadata(external.path()), external_before);
        jj(root, &["describe", "-m", "external source advance"]);
        fs::write(
            root.join("Note.md"),
            "dirty external bytes remain unsnapshotted",
        )
        .unwrap();
        let advanced = metadata(root);
        let external_advanced = metadata(external.path());
        let info = read(
            &f,
            client.client_id,
            "read",
            json!({"vault":token,"handle":handle,"query":{"kind":"query_info"}}),
        )
        .await;
        assert_eq!(info["kind"], "query_info");
        let newer = read(&f, client.client_id, "heads", json!({"vault":token})).await;
        assert_ne!(newer["heads"], heads["heads"]);
        assert_eq!(newer["generation"], heads["generation"]);
        let status = read(
            &f,
            client.client_id,
            "status",
            json!({"vault":token,"handle":handle}),
        )
        .await;
        assert_eq!(status["data"]["operation"], heads["heads"][0]);
        assert_eq!(metadata(root), advanced);
        assert_eq!(metadata(external.path()), external_advanced);
        assert_eq!(
            fs::read_to_string(root.join("Note.md")).unwrap(),
            "dirty external bytes remain unsnapshotted"
        );
        f.detach(&client).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn altered_or_symlinked_original_witnesses_remain_revoked_when_restored() {
    use std::os::unix::fs::symlink;
    for name in ["type", "git_target"] {
        let f = Fixture::with_database().await;
        let root = f.roots[0].path();
        init(root);
        let client = f.attach(f.ids[0]).await;
        let token = sources(&f, client.client_id, "open_original").await["token"].clone();
        let handle = prepared(&f, client.client_id, &token).await;
        let path = root.join(".jj/repo/store").join(name);
        let retained = tempfile::tempdir().unwrap();
        let original = retained.path().join("original");
        fs::rename(&path, &original).unwrap();
        symlink(&original, &path).unwrap();
        assert_eq!(
            history(
                &f,
                client.client_id,
                "status",
                json!({"vault":token,"handle":handle})
            )
            .await
            .status(),
            409
        );
        fs::remove_file(&path).unwrap();
        fs::rename(&original, &path).unwrap();
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
        assert!(
            sources(&f, fresh.client_id, "open_original").await["original"]
                .as_bool()
                .unwrap()
        );
        f.detach(&fresh).await;
    }
}
