use super::{
    support::*,
    tracked::{policy, raw},
};
use crate::services::indexing::owner::Clock;
use crate::services::{
    TrackedTaskError, TrackedTaskProvider, VaultService, daily::DailyDocumentProviderRegistry,
};
use lapis_task_services::scan_task_occurrence_observations;
use std::{
    fs,
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

async fn clock(vault: &Arc<VaultService>) -> Arc<Mutex<String>> {
    let date = Arc::new(Mutex::new("2026-08-20".to_string()));
    let current = date.clone();
    vault
        .start_indexing(
            false,
            true,
            Clock {
                today: Arc::new(move || current.lock().unwrap().clone()),
                interval: Duration::from_millis(25),
            },
        )
        .await
        .unwrap();
    until(async || vault.indexing_status().unwrap().passes > 0).await;
    date
}
async fn select(vault: &Arc<VaultService>) {
    vault
        .set_tracked_task_materialization(Some(TrackedTaskProvider::Default(policy())))
        .await
        .unwrap();
}
async fn failed(vault: &Arc<VaultService>) {
    until(async || {
        vault
            .indexing_status()
            .unwrap()
            .tracked
            .as_ref()
            .is_some_and(|status| !status.failures.is_empty())
    })
    .await;
}
async fn complete(vault: &Arc<VaultService>) {
    until(async || {
        vault
            .indexing_status()
            .unwrap()
            .tracked
            .as_ref()
            .is_some_and(|status| status.materialized == 1 && status.failures.is_empty())
    })
    .await;
}

#[tokio::test]
async fn tracked_selection_wakes_existing_owner_and_deselection_fences_new_dates() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Habit.md", &raw());
    write(
        &root,
        "Ignored.json",
        &raw().replace("id: tracked", "id: opaque"),
    );
    write(
        &root,
        ".hidden/Secret.md",
        &raw().replace("id: tracked", "id: hidden"),
    );
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let date = clock(&vault).await;
    assert!(!root.path().join("daily").exists());
    select(&vault).await;
    complete(&vault).await;
    let daily = fs::read_to_string(root.path().join("daily/2026-08-20.md")).unwrap();
    assert_eq!(
        scan_task_occurrence_observations(&daily, "daily/2026-08-20.md").len(),
        1
    );
    vault.set_tracked_task_materialization(None).await.unwrap();
    *date.lock().unwrap() = "2026-08-21".into();
    vault.set_tracked_task_materialization(None).await.unwrap();
    until(async || row(&vault, "Habit.md").await["effectiveForDate"] == "2026-08-21").await;
    assert!(!root.path().join("daily/2026-08-21.md").exists());
    write(&root, "Note.md", "# Ordinary metadata continues\n");
    let database = vault.database().await.unwrap();
    until(async || {
        database
            .get_indexed_file("Note.md")
            .await
            .unwrap()
            .is_some()
    })
    .await;
    select(&vault).await;
    until(async || root.path().join("daily/2026-08-21.md").exists()).await;
    assert_eq!(
        fs::read_to_string(root.path().join("Habit.md")).unwrap(),
        raw()
    );
    close(runtime).await;
}

#[tokio::test]
async fn tracked_provider_unavailability_is_visible_and_never_falls_back() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Habit.md", &raw());
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    clock(&vault).await;
    let registry = DailyDocumentProviderRegistry::<()>::default();
    let unavailable = registry.resolve().unwrap_err();
    vault
        .set_tracked_task_materialization(Some(TrackedTaskProvider::Unavailable(unavailable)))
        .await
        .unwrap();
    failed(&vault).await;
    let status = vault.indexing_status().unwrap();
    let failure = &status.tracked.as_ref().unwrap().failures[0];
    assert!(matches!(&*failure.cause, TrackedTaskError::Provider(_)));
    assert_eq!(
        failure.cause.to_string(),
        "No daily document provider is registered."
    );
    assert!(failure.retained_daily.is_none());
    assert!(!root.path().join("daily").exists());
    select(&vault).await;
    complete(&vault).await;
    assert!(vault.indexing_status().unwrap().error.is_none());
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
}

#[tokio::test]
async fn tracked_dirty_detached_original_and_alias_are_guarded_before_daily_creation() {
    for alias in [false, true] {
        let profile = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        write(&root, "Habit.md", &raw());
        if alias {
            fs::hard_link(root.path().join("Habit.md"), root.path().join("Alias.txt")).unwrap();
        }
        let runtime = runtime(&profile);
        let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
        let client = vault.attach().unwrap();
        let editor = client
            .open_editor(if alias { "Alias.txt" } else { "Habit.md" })
            .unwrap();
        client.mark_dirty(editor, 1).unwrap();
        drop(client);
        clock(&vault).await;
        select(&vault).await;
        failed(&vault).await;
        let status = vault.indexing_status().unwrap();
        assert!(matches!(
            &*status.tracked.unwrap().failures[0].cause,
            TrackedTaskError::Source(_)
        ));
        assert!(!root.path().join("daily").exists());
        let client = vault.attach().unwrap();
        client.recover_editor(editor).unwrap();
        client.discard_editor(editor).unwrap();
        if alias {
            fs::remove_file(root.path().join("Alias.txt")).unwrap();
        }
        select(&vault).await;
        complete(&vault).await;
        assert_eq!(
            fs::read_to_string(root.path().join("Habit.md")).unwrap(),
            raw()
        );
        drop(client);
        close(runtime).await;
    }
}

#[tokio::test]
async fn tracked_dirty_daily_and_alias_keep_readable_content_then_retry_one_observation() {
    for alias in [false, true] {
        let profile = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let daily = "---\ntype: daily-note\ndate: 2026-08-20\n---\n\n# Authored day\n\nHandwritten notes.\n";
        write(&root, "Habit.md", &raw());
        write(&root, "Journal/Existing.md", daily);
        if alias {
            fs::hard_link(
                root.path().join("Journal/Existing.md"),
                root.path().join("Alias.txt"),
            )
            .unwrap();
        }
        let runtime = runtime(&profile);
        let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
        let client = vault.attach().unwrap();
        let editor = client
            .open_editor(if alias {
                "Alias.txt"
            } else {
                "Journal/Existing.md"
            })
            .unwrap();
        client.mark_dirty(editor, 1).unwrap();
        drop(client);
        clock(&vault).await;
        select(&vault).await;
        failed(&vault).await;
        let status = vault.indexing_status().unwrap();
        let failure = &status.tracked.as_ref().unwrap().failures[0];
        assert!(matches!(&*failure.cause, TrackedTaskError::Daily(_)));
        assert_eq!(failure.retained_daily.as_ref().unwrap().text, daily);
        assert_eq!(
            fs::read_to_string(root.path().join("Journal/Existing.md")).unwrap(),
            daily
        );
        let client = vault.attach().unwrap();
        client.recover_editor(editor).unwrap();
        client.discard_editor(editor).unwrap();
        if alias {
            fs::remove_file(root.path().join("Alias.txt")).unwrap();
        }
        select(&vault).await;
        complete(&vault).await;
        let written = fs::read_to_string(root.path().join("Journal/Existing.md")).unwrap();
        assert!(written.contains("Handwritten notes."));
        assert_eq!(
            scan_task_occurrence_observations(&written, "Journal/Existing.md").len(),
            1
        );
        drop(client);
        close(runtime).await;
    }
}
