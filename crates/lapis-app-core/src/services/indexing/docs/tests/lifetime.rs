use super::support::*;
use lapis_app_protocol::{ErrorCode, VaultPhase};
use uuid::Uuid;

#[tokio::test]
async fn unattended_create_edit_rename_and_delete_converge_without_clients() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_docs_indexing("docs").await.unwrap();
    let database = database(&vault).await;
    until(async || {
        database
            .query_projection(ID, Default::default(), None)
            .await
            .is_ok()
    })
    .await;
    write(&root, "docs/First.doc.md", &note("first", "First"));
    until(async || row(&database, "docs/First.doc.md").await["id"] == "first").await;
    write(&root, "docs/First.doc.md", &note("edited", "Edited"));
    until(async || row(&database, "docs/First.doc.md").await["id"] == "edited").await;
    std::fs::rename(
        root.path().join("docs/First.doc.md"),
        root.path().join("docs/Renamed.doc.md"),
    )
    .unwrap();
    until(async || row(&database, "docs/Renamed.doc.md").await["path"] == "docs/Renamed.doc.md")
        .await;
    assert!(row(&database, "docs/First.doc.md").await.is_null());
    std::fs::remove_file(root.path().join("docs/Renamed.doc.md")).unwrap();
    until(async || {
        database
            .query_projection(ID, Default::default(), None)
            .await
            .unwrap()
            .rows
            .is_empty()
    })
    .await;
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
}

#[tokio::test]
async fn disable_fences_docs_while_markdown_continues_and_reenable_repairs() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "docs/Note.doc.md", &note("before", "Before"));
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_docs_indexing("docs").await.unwrap();
    let database = database(&vault).await;
    until(async || row(&database, "docs/Note.doc.md").await["id"] == "before").await;
    let old_hash = database
        .native_projection_state(ID, "docs/Note.doc.md")
        .await
        .unwrap()
        .source
        .unwrap()
        .hash;
    vault.stop_docs_indexing().unwrap();
    let changed = note("after", "After");
    write(&root, "docs/Note.doc.md", &changed);
    until(async || {
        database
            .get_indexed_file("docs/Note.doc.md")
            .await
            .unwrap()
            .unwrap()
            .file
            .hash
            == lapis_plugin_services::markdown::content_hash(&changed)
    })
    .await;
    assert!(row(&database, "docs/Note.doc.md").await.is_null());
    assert_eq!(
        database
            .native_projection_state(ID, "docs/Note.doc.md")
            .await
            .unwrap()
            .source
            .unwrap()
            .hash,
        old_hash
    );
    vault.start_docs_indexing("docs").await.unwrap();
    until(async || row(&database, "docs/Note.doc.md").await["id"] == "after").await;
    close(runtime).await;
}

#[tokio::test]
async fn warm_and_cold_unchanged_sources_do_not_append_database_revisions() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    write(&root, "docs/Note.doc.md", &note("note", "Note"));
    let first = runtime(&profile);
    let vault = first.admit(id, root.path()).unwrap();
    vault.start_docs_indexing("docs").await.unwrap();
    let database = database(&vault).await;
    until(async || row(&database, "docs/Note.doc.md").await["id"] == "note").await;
    let revision = database.revision().await.unwrap();
    rescan(&vault, "docs").await;
    assert_eq!(database.revision().await.unwrap(), revision);
    close(first).await;
    let next = runtime(&profile);
    let vault = next.admit(id, root.path()).unwrap();
    vault.start_docs_indexing("docs").await.unwrap();
    until(async || vault.indexing_status().unwrap().passes > 0).await;
    let database = super::support::database(&vault).await;
    assert_eq!(database.revision().await.unwrap(), revision);
    close(next).await;
}

#[tokio::test]
async fn same_stat_external_edits_and_periodic_cache_repair_remain_unattended() {
    use std::{sync::Arc, time::Duration};
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "docs/Note.doc.md", &note("note", "Before"));
    let path = root.path().join("docs/Note.doc.md");
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault
        .start_indexing(
            false,
            false,
            super::super::super::owner::Clock {
                today: Arc::new(|| "2026-10-25".into()),
                interval: Duration::from_millis(30),
            },
        )
        .await
        .unwrap();
    vault.start_docs_indexing("docs").await.unwrap();
    let database = database(&vault).await;
    until(async || row(&database, "docs/Note.doc.md").await["title"] == "Before").await;
    write(&root, "docs/Note.doc.md", &note("note", "After!"));
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    until(async || row(&database, "docs/Note.doc.md").await["title"] == "After!").await;
    database
        .delete_projection_source(ID, "docs/Note.doc.md", Some("docs"))
        .await
        .unwrap();
    until(async || row(&database, "docs/Note.doc.md").await["title"] == "After!").await;
    close(runtime).await;
}

#[tokio::test]
async fn concurrent_native_selection_and_close_drain_one_owner_without_reopening_it() {
    for _ in 0..4 {
        let profile = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        write(&root, "docs/Note.doc.md", &note("note", "Note"));
        let runtime = runtime(&profile);
        let id = Uuid::new_v4();
        let vault = runtime.admit(id, root.path()).unwrap();
        let closing = runtime.clone();
        let (start, closed) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(
                vault.start_docs_indexing("docs"),
                tokio::task::spawn_blocking(move || closing.close_vault(id))
            )
        })
        .await
        .expect("Docs selection/close must not deadlock");
        if let Err(error) = start {
            assert!(matches!(error.code(), ErrorCode::Closed | ErrorCode::Busy));
        }
        closed.unwrap().unwrap();
        assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
        assert_eq!(vault.status().unwrap().work, 0);
        assert_eq!(
            vault.start_docs_indexing("docs").await.unwrap_err().code(),
            ErrorCode::Closed
        );
        let database = crate::database::Database::open(profile.path(), id)
            .await
            .unwrap();
        database.close().await.unwrap();
    }
}
