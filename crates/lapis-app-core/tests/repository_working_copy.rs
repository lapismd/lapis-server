use lapis_app_core::services::Runtime;
use std::{fs, path::Path, process::Command, sync::Arc};
use uuid::Uuid;

fn jj(root: &Path, args: &[&str]) {
    let output = Command::new("jj")
        .current_dir(root)
        .args([
            "--no-pager",
            "--config",
            "user.name='Working copy fixture'",
            "--config",
            "user.email='fixture@invalid'",
            "--config",
            "signing.behavior='drop'",
        ])
        .args(args)
        .env("JJ_CONFIG", "/dev/null")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn heads(root: &Path) -> Vec<(String, Vec<u8>)> {
    let directory = root.join(".jj/repo/op_heads");
    let mut pending = vec![directory.clone()];
    let mut result = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                pending.push(entry.path());
            } else {
                result.push((
                    entry
                        .path()
                        .strip_prefix(&directory)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned(),
                    fs::read(entry.path()).unwrap(),
                ));
            }
        }
    }
    result.sort();
    result
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn original_working_copy_counts_saved_unsnapshotted_files_without_capturing_or_moving_history()
 {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    jj(&root, &["git", "init", "--no-colocate", "."]);
    fs::write(root.join("Note.md"), "before\n").unwrap();
    jj(&root, &["commit", "-m", "Initial saved version"]);
    fs::write(root.join("Note.md"), "after\n").unwrap();
    fs::write(root.join("Extra.md"), "unsnapshotted\n").unwrap();
    let before = heads(&root);
    let checkout = fs::read(root.join(".jj/working_copy/checkout")).unwrap();
    let tree = fs::read(root.join(".jj/working_copy/tree_state")).unwrap();
    let runtime = Arc::new(Runtime::default());
    let vault = runtime.admit(Uuid::new_v4(), &root).unwrap();
    let client = vault.attach().unwrap();
    let frame = client.repository_working_copy().await.unwrap();
    let status: serde_json::Value = serde_json::from_str(&frame).unwrap();
    assert_eq!(status["version"], 1);
    assert_eq!(status["source"], "original");
    assert_eq!(status["files"], 2);
    assert_eq!(
        status["lines"],
        serde_json::json!({"additions":2,"deletions":1})
    );
    assert_eq!(heads(&root), before);
    assert_eq!(
        fs::read(root.join(".jj/working_copy/checkout")).unwrap(),
        checkout
    );
    assert_eq!(
        fs::read(root.join(".jj/working_copy/tree_state")).unwrap(),
        tree
    );
    assert_eq!(
        fs::read_to_string(root.join("Extra.md")).unwrap(),
        "unsnapshotted\n"
    );
    client.detach().unwrap();
    assert!(client.repository_working_copy().await.is_err());
    let client = vault.attach().unwrap();
    assert!(client.repository_working_copy().await.is_ok());
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn working_copy_read_without_an_original_never_initializes_a_repository() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    fs::write(root.join("Note.md"), "saved bytes").unwrap();
    let runtime = Arc::new(Runtime::default());
    let vault = runtime.admit(Uuid::new_v4(), &root).unwrap();
    let client = vault.attach().unwrap();
    assert!(client.repository_working_copy().await.is_err());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    assert_eq!(vault.status().unwrap().work, 0);
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
