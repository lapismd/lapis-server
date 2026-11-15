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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn backup_status_reads_the_recorded_checkpoint_without_capturing_new_disk_bytes() {
    use jj_store_jj::vault_workspace::{Binding, HistoryReader, Session};
    use lapis_app_core::services::BackupStorage;
    for binary in [false, true] {
        let temporary = tempfile::tempdir().unwrap();
        let base = temporary.path().canonicalize().unwrap();
        let root = base.join("notes");
        let history = base.join("history");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&history).unwrap();
        let id = Uuid::new_v4();
        let binding = Binding {
            vault_id: id.to_string(),
            root: root.clone(),
            state: history.join(id.to_string()),
        };
        fs::write(root.join("Note.md"), "before\n").unwrap();
        let mut capture = Session::open(binding.clone()).await.unwrap();
        capture
            .capture(&[], "Initial saved checkpoint")
            .await
            .unwrap();
        fs::write(root.join("Note.md"), "after\n").unwrap();
        fs::write(root.join("Extra.md"), "saved addition\n").unwrap();
        if binary {
            fs::write(root.join("binary.dat"), [0u8, 255, 1]).unwrap();
        }
        let saved = capture
            .capture(&[], "Reviewed local checkpoint")
            .await
            .unwrap()
            .checkpoint
            .unwrap();
        let before = capture.operation();
        fs::write(root.join("Note.md"), "later unrecorded bytes\n").unwrap();
        fs::write(
            root.join("NotCaptured.md"),
            "must remain outside the checkpoint\n",
        )
        .unwrap();
        let runtime =
            Arc::new(Runtime::default().with_backup(BackupStorage::new(&history).unwrap()));
        let vault = runtime.admit(id, &root).unwrap();
        let client = vault.attach().unwrap();
        let status: serde_json::Value =
            serde_json::from_str(&client.repository_working_copy().await.unwrap()).unwrap();
        assert_eq!(status["source"], "backup");
        assert_eq!(status["working_copy"], saved.commit);
        assert_eq!(status["commit"]["description"], "Reviewed local checkpoint");
        assert_eq!(status["files"], if binary { 3 } else { 2 });
        assert_eq!(status["uncounted"], if binary { 1 } else { 0 });
        if binary {
            assert!(status["lines"].is_null());
        } else {
            assert_eq!(
                status["lines"],
                serde_json::json!({"additions":2,"deletions":1})
            );
        }
        assert_eq!(capture.operation(), before);
        assert!(!root.join(".jj").exists());
        assert_eq!(
            fs::read_to_string(root.join("Note.md")).unwrap(),
            "later unrecorded bytes\n"
        );
        assert_eq!(
            HistoryReader::open(binding)
                .await
                .unwrap()
                .history_revision(None, 0)
                .await
                .unwrap()
                .revision
                .unwrap()
                .checkpoint
                .commit,
            saved.commit
        );
        client.detach().unwrap();
        assert!(client.repository_working_copy().await.is_err());
        tokio::task::spawn_blocking(move || runtime.shutdown())
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_invalid_original_never_falls_back_to_an_existing_backup_checkpoint() {
    use jj_store_jj::vault_workspace::{Binding, Session};
    use lapis_app_core::services::BackupStorage;
    let temporary = tempfile::tempdir().unwrap();
    let base = temporary.path().canonicalize().unwrap();
    let root = base.join("notes");
    let history = base.join("history");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&history).unwrap();
    let id = Uuid::new_v4();
    let binding = Binding {
        vault_id: id.to_string(),
        root: root.clone(),
        state: history.join(id.to_string()),
    };
    fs::write(root.join("Note.md"), "recorded bytes\n").unwrap();
    let mut capture = Session::open(binding).await.unwrap();
    capture.capture(&[], "Existing backup").await.unwrap();
    fs::create_dir(root.join(".jj")).unwrap();
    fs::write(
        root.join(".jj/invalid-marker"),
        "retain this invalid original",
    )
    .unwrap();
    let runtime = Arc::new(Runtime::default().with_backup(BackupStorage::new(&history).unwrap()));
    let vault = runtime.admit(id, &root).unwrap();
    let client = vault.attach().unwrap();
    assert!(client.repository_working_copy().await.is_err());
    assert_eq!(
        fs::read_to_string(root.join(".jj/invalid-marker")).unwrap(),
        "retain this invalid original"
    );
    assert_eq!(fs::read_dir(root.join(".jj")).unwrap().count(), 1);
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
