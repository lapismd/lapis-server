use super::support::*;
use lapis_app_protocol::{ErrorCode, VaultPhase, database::call::DatabaseMethod};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn missing_non_task_deleted_and_cold_sources_repair_without_metadata_or_revision_churn() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    write(&root, "Task.md", &document("task", "task", ""));
    let first = runtime(&profile);
    let vault = first.admit(id, root.path()).unwrap();
    vault.set_task_document_indexing(true).await.unwrap();
    until(async || !row(&vault, "Task.md").await.is_null()).await;
    let revision = query(&vault).await.unwrap()["revision"].clone();
    rescan(&vault, &root).await;
    assert_eq!(query(&vault).await.unwrap()["revision"], revision);
    call(
        &vault,
        DatabaseMethod::DeleteProjectionSource,
        vec![json!("tasks/task"), json!("Task.md"), json!("tasks")],
    )
    .await
    .unwrap();
    rescan(&vault, &root).await;
    until(async || !row(&vault, "Task.md").await.is_null()).await;
    let repaired = query(&vault).await.unwrap()["revision"].clone();
    close(first).await;
    let next = runtime(&profile);
    let vault = next.admit(id, root.path()).unwrap();
    vault.set_task_document_indexing(true).await.unwrap();
    until(async || vault.indexing_status().unwrap().passes > 0).await;
    assert_eq!(query(&vault).await.unwrap()["revision"], repaired);
    write(&root, "Task.md", "# Ordinary now\n");
    until(async || row(&vault, "Task.md").await.is_null()).await;
    write(&root, "Task.md", &document("again", "task", ""));
    until(async || row(&vault, "Task.md").await["documentId"] == "again").await;
    std::fs::remove_file(root.path().join("Task.md")).unwrap();
    until(async || query(&vault).await.unwrap()["rows"] == json!([])).await;
    close(next).await;
}

#[tokio::test]
async fn disabling_fences_task_updates_while_markdown_continues_and_reenable_repairs() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Task.md", &document("first", "task", ""));
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.set_task_document_indexing(true).await.unwrap();
    until(async || row(&vault, "Task.md").await["documentId"] == "first").await;
    vault.set_task_document_indexing(false).await.unwrap();
    write(&root, "Task.md", &document("next", "task", ""));
    until(async || vault.indexing_status().unwrap().indexed >= 2).await;
    assert!(row(&vault, "Task.md").await.is_null());
    vault.set_task_document_indexing(true).await.unwrap();
    until(async || row(&vault, "Task.md").await["documentId"] == "next").await;
    close(runtime).await;
}

#[tokio::test]
async fn concurrent_private_selection_and_close_drain_without_a_retired_scheduler() {
    for _ in 0..4 {
        let profile = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        write(&root, "Task.md", &document("task", "task", ""));
        let runtime = runtime(&profile);
        let id = Uuid::new_v4();
        let vault = runtime.admit(id, root.path()).unwrap();
        let closing = runtime.clone();
        let (start, closed) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(
                vault.set_task_document_indexing(true),
                tokio::task::spawn_blocking(move || closing.close_vault(id))
            )
        })
        .await
        .expect("selection/close must not deadlock");
        if let Err(error) = start {
            assert!(matches!(error.code(), ErrorCode::Closed | ErrorCode::Busy));
        }
        closed.unwrap().unwrap();
        assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
        assert_eq!(vault.status().unwrap().work, 0);
        assert_eq!(
            vault
                .set_task_document_indexing(true)
                .await
                .unwrap_err()
                .code(),
            ErrorCode::Closed
        );
        let database = crate::database::Database::open(profile.path(), id)
            .await
            .unwrap();
        database.close().await.unwrap();
    }
}
