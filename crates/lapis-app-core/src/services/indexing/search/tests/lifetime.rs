use super::support::*;
use crate::database::Database;
use lapis_app_protocol::VaultPhase;
use uuid::Uuid;

#[tokio::test]
async fn disable_fences_search_while_markdown_continues_and_reenable_repairs_it() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Note.md", "# Before\n");
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    select(&vault, true).await;
    let db = database(&vault).await;
    until(async || db.get_search_document("Note.md").await.unwrap().is_some()).await;
    select(&vault, false).await;
    write(&root, "Note.md", "# After!\n");
    until(async || {
        db.get_indexed_file("Note.md")
            .await
            .unwrap()
            .unwrap()
            .metadata
            .unwrap()
            .metadata["headings"][0]["heading"]
            == "After!"
    })
    .await;
    assert_eq!(
        db.get_search_document("Note.md")
            .await
            .unwrap()
            .unwrap()
            .content,
        "# Before\n"
    );
    rescan(&vault).await;
    assert_eq!(
        db.get_search_document("Note.md")
            .await
            .unwrap()
            .unwrap()
            .content,
        "# After!\n"
    );
    close(runtime).await;
}

#[tokio::test]
async fn cold_reopen_detects_same_stat_edits_and_repairs_search_without_rewriting_notes() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Note.md", "# Before\n");
    let path = root.path().join("Note.md");
    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
    let id = Uuid::new_v4();
    let first = runtime(&profile);
    let old = first.admit(id, root.path()).unwrap();
    select(&old, true).await;
    let db = database(&old).await;
    until(async || db.get_search_document("Note.md").await.unwrap().is_some()).await;
    close(first).await;
    write(&root, "Note.md", "# After!\n");
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(mtime))
        .unwrap();
    let second = runtime(&profile);
    let current = second.admit(id, root.path()).unwrap();
    select(&current, true).await;
    let db = database(&current).await;
    until(async || {
        db.get_search_document("Note.md")
            .await
            .unwrap()
            .is_some_and(|row| row.content == "# After!\n")
    })
    .await;
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "# After!\n");
    close(second).await;
}

#[tokio::test]
async fn hidden_and_symlink_sources_never_enter_search_and_close_drains_the_owner() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(&root, "Note.md", "# Visible\n");
    write(&root, ".lapis/Hidden.md", "# Hidden\n");
    write(&outside, "Secret.md", "# Secret\n");
    std::os::unix::fs::symlink(
        outside.path().join("Secret.md"),
        root.path().join("Escape.md"),
    )
    .unwrap();
    let runtime = runtime(&profile);
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, root.path()).unwrap();
    select(&vault, true).await;
    let db = database(&vault).await;
    until(async || db.get_search_index_stats().await.unwrap().document_count == 1).await;
    assert!(
        db.get_search_document(".lapis/Hidden.md")
            .await
            .unwrap()
            .is_none()
    );
    assert!(db.get_search_document("Escape.md").await.unwrap().is_none());
    close(runtime).await;
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
    assert_eq!(vault.status().unwrap().work, 0);
    let reopened = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(
        reopened
            .get_search_index_stats()
            .await
            .unwrap()
            .document_count,
        1
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn concurrent_start_stop_and_close_never_leave_a_retired_search_owner() {
    for _ in 0..4 {
        let profile = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        write(&root, "Note.md", "# Source\n");
        let runtime = runtime(&profile);
        let id = Uuid::new_v4();
        let vault = runtime.admit(id, root.path()).unwrap();
        let closing = runtime.clone();
        let (start, closed) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(
                vault.start_search_indexing(),
                tokio::task::spawn_blocking(move || closing.close_vault(id))
            )
        })
        .await
        .expect("Search selection and close must not deadlock");
        if let Err(error) = start {
            assert!(matches!(
                error.code(),
                lapis_app_protocol::ErrorCode::Closed | lapis_app_protocol::ErrorCode::Busy
            ));
        }
        closed.unwrap().unwrap();
        assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
        assert_eq!(vault.status().unwrap().work, 0);
        assert_eq!(
            vault.start_search_indexing().await.unwrap_err().code(),
            lapis_app_protocol::ErrorCode::Closed
        );
        assert_eq!(
            vault.stop_search_indexing().unwrap_err().code(),
            lapis_app_protocol::ErrorCode::Closed
        );
        let database = Database::open(profile.path(), id).await.unwrap();
        database.close().await.unwrap();
    }
}
