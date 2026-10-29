use jj_vault::{RestoreRequest, VaultRestoreRequest};
use lapis_app_core::services::{BackupStorage, Client, Runtime};
use lapis_app_protocol::{ErrorCode, repository_restore::*};
use std::{fs, path::PathBuf, sync::Arc};
use uuid::Uuid;

const FILE_STALE: &str = "The file changed since the restore preview; review it again";
const VAULT_STALE: &str = "Vault files changed since the restore preview; review it again";
const UNAVAILABLE: &str = "Automatic vault backup is unavailable";
const UNCERTAIN: &str =
    "Repository action did not complete; review current files or retry the same pending restore";

struct Fixture {
    _temp: tempfile::TempDir,
    runtime: Arc<Runtime>,
    client: Arc<Client>,
    root: PathBuf,
    checkpoint: String,
}
async fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("notes");
    let history = base.join("history");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&history).unwrap();
    fs::write(root.join("Note.md"), "initial saved bytes").unwrap();
    let runtime = Arc::new(Runtime::default().with_backup(BackupStorage::new(&history).unwrap()));
    let client = Arc::new(
        runtime
            .admit(Uuid::new_v4(), &root)
            .unwrap()
            .attach()
            .unwrap(),
    );
    let status = client.start_backup().await.unwrap();
    let checkpoint = status.borrow().checkpoint.as_ref().unwrap().commit.clone();
    Fixture {
        _temp: temp,
        runtime,
        client,
        root,
        checkpoint,
    }
}
async fn close(f: Fixture) {
    tokio::task::spawn_blocking(move || f.runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
async fn file_request(f: &Fixture) -> RestoreRequest {
    let checkpoint = f.checkpoint.clone();
    let preview = f
        .client
        .with_backup(move |engine| {
            Box::pin(async move { engine.preview_restore(checkpoint, "Note.md".into()).await })
        })
        .await
        .unwrap();
    RestoreRequest {
        request_id: Uuid::new_v4().to_string(),
        checkpoint: f.checkpoint.clone(),
        path: preview.path,
        expected: preview.expected,
    }
}
async fn apply_file(
    f: &Fixture,
    request: RestoreRequest,
    typed: bool,
) -> lapis_app_core::services::Result<()> {
    if typed {
        f.client
            .control_repository_restore(RepositoryRestoreCommand::Apply {
                request: RepositoryFileRestoreRequest {
                    request_id: request.request_id,
                    checkpoint: request.checkpoint,
                    path: request.path,
                    expected: request.expected,
                },
            })
            .await
            .map(|_| ())
    } else {
        f.client
            .with_backup(move |engine| Box::pin(async move { engine.restore(request).await }))
            .await
            .map(|_| ())
    }
}
async fn stale_file(typed: bool) {
    let f = fixture().await;
    fs::write(f.root.join("Note.md"), "saved at preview").unwrap();
    let request = file_request(&f).await;
    fs::write(f.root.join("Note.md"), "unreviewed saved bytes").unwrap();
    let error = apply_file(&f, request, typed).await.unwrap_err();
    assert_eq!(error.code(), ErrorCode::Conflict);
    assert_eq!(error.to_string(), FILE_STALE);
    assert_eq!(
        fs::read_to_string(f.root.join("Note.md")).unwrap(),
        "unreviewed saved bytes"
    );
    close(f).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn borrowed_restore_preserves_the_exact_stale_file_guard_and_saved_bytes() {
    stale_file(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_restore_preserves_the_exact_stale_file_guard_and_saved_bytes() {
    stale_file(true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn borrowed_and_typed_vault_restore_preserve_the_stale_tree_guard() {
    let f = fixture().await;
    fs::write(f.root.join("Note.md"), "saved at vault preview").unwrap();
    let checkpoint = f.checkpoint.clone();
    let preview = f
        .client
        .with_backup(move |engine| {
            Box::pin(async move { engine.preview_vault_restore(checkpoint).await })
        })
        .await
        .unwrap();
    fs::write(f.root.join("Note.md"), "newer vault bytes").unwrap();
    fs::write(f.root.join("Added.md"), "new file after preview").unwrap();
    for typed in [false, true] {
        let request = VaultRestoreRequest {
            request_id: Uuid::new_v4().to_string(),
            checkpoint: f.checkpoint.clone(),
            expected: preview.expected.clone(),
        };
        let error = if typed {
            f.client
                .control_repository_restore(RepositoryRestoreCommand::ApplyVault {
                    request: RepositoryVaultRestoreRequest {
                        request_id: request.request_id,
                        checkpoint: request.checkpoint,
                        expected: request.expected,
                    },
                })
                .await
                .map(|_| ())
        } else {
            f.client
                .with_backup(move |engine| {
                    Box::pin(async move { engine.restore_vault(request).await })
                })
                .await
                .map(|_| ())
        }
        .unwrap_err();
        assert_eq!(error.code(), ErrorCode::Conflict);
        assert_eq!(error.to_string(), VAULT_STALE);
        assert_eq!(
            fs::read_to_string(f.root.join("Note.md")).unwrap(),
            "newer vault bytes"
        );
        assert_eq!(
            fs::read_to_string(f.root.join("Added.md")).unwrap(),
            "new file after preview"
        );
    }
    close(f).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn borrowed_and_typed_restore_keep_the_unresolved_editor_guard_actionable() {
    let f = fixture().await;
    fs::write(f.root.join("Note.md"), "current saved bytes").unwrap();
    let editor = f.client.open_editor("Note.md").unwrap();
    f.client.mark_dirty(editor, 1).unwrap();
    for typed in [false, true] {
        let request = file_request(&f).await;
        let error = apply_file(&f, request, typed).await.unwrap_err();
        assert_eq!(error.code(), ErrorCode::Busy);
        assert_eq!(error.to_string(), "Vault has unresolved editor state");
        assert_eq!(
            fs::read_to_string(f.root.join("Note.md")).unwrap(),
            "current saved bytes"
        );
    }
    f.client.discard_editor(editor).unwrap();
    close(f).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_the_exact_fixed_restore_diagnostics_cross_the_borrowed_boundary() {
    let f = fixture().await;
    for (message, code) in [
        (FILE_STALE, ErrorCode::Conflict),
        (VAULT_STALE, ErrorCode::Conflict),
        (
            "Vault files changed before restoration could start",
            ErrorCode::Conflict,
        ),
        (
            "Files changed while restoring the vault; later edits were retained",
            ErrorCode::Conflict,
        ),
        ("Vault has unresolved editor state", ErrorCode::Busy),
        (
            "This file is excluded from vault backup",
            ErrorCode::PermissionDenied,
        ),
        (
            "Restore of symbolic links is unavailable",
            ErrorCode::Unsupported,
        ),
        (
            "Vault restore reached its work limit; retry to continue the same restore",
            ErrorCode::Busy,
        ),
    ] {
        for native in [message.to_owned(), format!("Vault backup JJ: {message}")] {
            let error = f
                .client
                .with_backup(move |_| Box::pin(async move { Err::<(), _>(native) }))
                .await
                .unwrap_err();
            assert_eq!(error.code(), code);
            assert_eq!(error.to_string(), message);
        }
    }
    close(f).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn arbitrary_engine_io_ownership_and_credential_details_remain_redacted() {
    let f = fixture().await;
    for message in [
        "Permission denied: /private/vault/secret.md".to_owned(),
        "https://operator:password@example.invalid/private-repo failed".into(),
        "Vault service is closed".into(),
        "Native history storage is not selected".into(),
        format!("{FILE_STALE}\n/private/secret"),
        format!("IO error: {FILE_STALE}"),
        format!("Vault backup IO: {FILE_STALE}"),
        format!("Vault backup JJ: {FILE_STALE}\n/private/secret"),
        format!("Vault backup JJ: Vault backup JJ: {FILE_STALE}"),
    ] {
        let error = f
            .client
            .with_backup(move |_| Box::pin(async move { Err::<(), _>(message) }))
            .await
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::Io);
        assert_eq!(error.to_string(), UNAVAILABLE);
    }
    close(f).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_restore_unknown_native_failure_keeps_its_generic_pending_receipt_message() {
    let f = fixture().await;
    fs::create_dir(f.root.join("private-directory")).unwrap();
    let error = f
        .client
        .control_repository_restore(RepositoryRestoreCommand::Preview {
            checkpoint: f.checkpoint.clone(),
            path: "private-directory".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::Conflict);
    assert_eq!(error.to_string(), UNCERTAIN);
    assert_eq!(
        fs::read_to_string(f.root.join("Note.md")).unwrap(),
        "initial saved bytes"
    );
    close(f).await;
}
