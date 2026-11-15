//! Real HTTP plus native SSH, launched only by the disposable endpoint harness.
use super::*;
use std::{fs, path::PathBuf, process::Command};
fn jj(root: &Path, args: &[&str]) -> String {
    let output = Command::new("jj")
        .current_dir(root)
        .args([
            "--no-pager",
            "--config",
            "user.name='HTTP fixture'",
            "--config",
            "user.email='fixture@invalid'",
            "--config",
            "signing.behavior='drop'",
        ])
        .args(args)
        .env("JJ_CONFIG", "/dev/null")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn remote(base: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .arg("--git-dir")
        .arg(base.join("remote.git"))
        .args(args)
        .output()
        .unwrap()
}
fn receives(base: &Path) -> usize {
    fs::read_to_string(base.join("receive-count"))
        .unwrap_or_default()
        .lines()
        .count()
}
fn target<'a>(value: &'a Value, target: &str) -> &'a Value {
    value["settings"]["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["target"] == target)
        .unwrap()
}
async fn command(
    f: &Running,
    c: &ClientDescriptor,
    kind: &str,
    selected: &str,
    fields: Value,
) -> reqwest::Response {
    let state = f.settings(c).await;
    let mut input = json!({"kind":kind, "target":selected, "expectedRevision":state["revision"]});
    input
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    f.control(c, input).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires scripts/test-repository-remotes-ssh.mjs --server-only"]
async fn http_ssh_fetch_review_and_lost_response_recover_one_exact_branch_per_target() {
    let base = PathBuf::from(std::env::var_os("LAPIS_REMOTES_SSH_FIXTURE").unwrap());
    let endpoint = std::env::var("LAPIS_REMOTES_SSH_ENDPOINT").unwrap();
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("vault-0");
    fs::create_dir(&original).unwrap();
    jj(&original, &["git", "init", "--no-colocate", "."]);
    fs::write(original.join("Note.md"), "reviewed original\n").unwrap();
    jj(&original, &["commit", "-m", "Selected HTTP publication"]);
    jj(&original, &["bookmark", "create", "publish", "-r", "@-"]);
    jj(
        &original,
        &["bookmark", "create", "never-publish-http", "-r", "@-"],
    );
    let commit = jj(
        &original,
        &["log", "-r", "publish", "--no-graph", "-T", "commit_id"],
    );
    fs::write(original.join("Uncaptured.md"), "do not publish this\n").unwrap();
    let checkout = fs::read(original.join(".jj/working_copy/checkout")).unwrap();
    let tree = fs::read(original.join(".jj/working_copy/tree_state")).unwrap();
    let trust = base.join("known 'hosts'");
    let ids = [Uuid::new_v4(), Uuid::new_v4()];
    let mut f = Running::start_with_trust(root.path(), None, ids, Some(&trust)).await;
    for (selected, vault, branch, bookmark) in [
        ("original", ids[0], "http-original", "publish"),
        ("backup", ids[1], "http-backup", ""),
    ] {
        let c = f.attach(vault).await;
        let before = receives(&base);
        let saved: Value = command(&f, &c, "save_connection", selected, json!({"connection":{
            "name":"http-remote", "url":endpoint, "branch":branch, "bookmark":bookmark, "authentication":"ssh-agent", "credential":"",
        }})).await.error_for_status().unwrap().json().await.unwrap();
        assert_eq!(saved["settings"]["storage"], "locked");
        assert_eq!(saved["settings"]["automaticSync"], false);
        assert_eq!(receives(&base), before);
        f.stop().await;
        f = Running::start_with_trust(
            root.path(),
            None,
            ids,
            Some(&base.join("wrong-known-hosts")),
        )
        .await;
        let rejected = f.attach(vault).await;
        assert!(
            !command(&f, &rejected, "test_connection", selected, json!({}))
                .await
                .status()
                .is_success()
        );
        assert_eq!(receives(&base), before, "wrong host keys never publish");
        f.stop().await;
        f = Running::start_with_trust(root.path(), None, ids, Some(&trust)).await;
        let c = f.attach(vault).await;
        let tested: Value = command(&f, &c, "test_connection", selected, json!({}))
            .await
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(target(&tested, selected)["observation"].is_null());
        if selected == "original" {
            fs::write(base.join("hold-next-observation"), "fixture").unwrap();
            let state = f.settings(&c).await;
            let request = f.auth(f.client.post(f.url(&c))).json(&json!({
                "kind":"fetch", "target":selected, "expectedRevision":state["revision"],
            }));
            let pending = tokio::spawn(async move { request.send().await });
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !base.join("observation-held").exists() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "fetch did not reach the real endpoint"
                );
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            pending.abort();
            let _ = pending.await;
            f.auth(
                f.client
                    .delete(format!("{}/api/v3/clients/{}", f.base, c.client_id)),
            )
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
            fs::remove_file(base.join("hold-next-observation")).unwrap();
            let replacement = f.attach(vault).await;
            let finished = f.settings(&replacement).await;
            assert!(
                target(&finished, selected)["observation"].is_object(),
                "accepted fetch survives waiter cancellation and detach"
            );
            assert_eq!(receives(&base), before);
        }
        let c = f.attach(vault).await;
        command(&f, &c, "fetch", selected, json!({}))
            .await
            .error_for_status()
            .unwrap();
        let reviewed: Value = command(&f, &c, "review_push", selected, json!({}))
            .await
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        let review = target(&reviewed, selected)["review"].clone();
        if selected == "original" {
            assert_eq!(review["commit"], commit);
        }
        assert_eq!(
            receives(&base),
            before,
            "save/test/fetch/review cannot publish"
        );
        fs::write(base.join("drop-next-response"), "fixture").unwrap();
        let response = command(&f, &c, "push", selected, json!({"reviewId":review["id"]})).await;
        if response.status().is_success() {
            let outcome: Value = response.json().await.unwrap();
            assert!(target(&outcome, selected)["publication"].is_object());
        }
        assert_eq!(receives(&base), before + 1);
        let pending = f.settings(&c).await;
        assert_eq!(
            target(&pending, selected)["publication"]["id"],
            review["id"]
        );
        f.stop().await;
        f = Running::start_with_trust(root.path(), None, ids, Some(&trust)).await;
        let next = f.attach(vault).await;
        let restored = f.settings(&next).await;
        assert_eq!(
            target(&restored, selected)["publication"]["id"],
            review["id"]
        );
        let reconciled: Value = command(
            &f,
            &next,
            "observe_push",
            selected,
            json!({"requestId":review["id"]}),
        )
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
        assert!(target(&reconciled, selected)["publication"].is_null());
        assert_eq!(
            receives(&base),
            before + 1,
            "restart recovery must not send again"
        );
        assert_eq!(
            command(
                &f,
                &next,
                "push",
                selected,
                json!({"reviewId":review["id"]})
            )
            .await
            .status(),
            409
        );
        let actual = remote(&base, &["rev-parse", &format!("refs/heads/{branch}")]);
        assert!(actual.status.success());
        assert_eq!(
            String::from_utf8(actual.stdout).unwrap().trim(),
            review["commit"].as_str().unwrap()
        );
        command(&f, &next, "fetch", selected, json!({}))
            .await
            .error_for_status()
            .unwrap();
        assert_eq!(receives(&base), before + 1);
        assert_eq!(
            fs::read(original.join(".jj/working_copy/checkout")).unwrap(),
            checkout
        );
        assert_eq!(
            fs::read(original.join(".jj/working_copy/tree_state")).unwrap(),
            tree
        );
        assert_eq!(
            fs::read_to_string(original.join("Uncaptured.md")).unwrap(),
            "do not publish this\n"
        );
        assert!(
            !remote(
                &base,
                &["show-ref", "--verify", "refs/heads/never-publish-http"]
            )
            .status
            .success()
        );
        assert!(
            !remote(
                &base,
                &["cat-file", "-e", &format!("{commit}:Uncaptured.md")]
            )
            .status
            .success()
        );
    }
    f.stop().await;
    assert!(!root.path().join("vault-1/.jj").exists());
}
