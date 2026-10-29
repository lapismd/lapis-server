use super::support::*;
use crate::services::indexing::owner::Clock;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

#[tokio::test]
async fn local_date_refresh_updates_unchanged_task_rows_without_clients_or_materialization() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let raw = "---\ntype: task\nid: repeating\ntask:\n  repeat:\n    start: 2026-08-20\n    rrule: FREQ=DAILY\n    anchor: schedule\n    missed: skip\n---\n\n# Repeat\n";
    write(&root, "Repeat.md", raw);
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let date = Arc::new(Mutex::new(String::from("2026-08-20")));
    let current = date.clone();
    let clock = Clock {
        today: Arc::new(move || current.lock().unwrap().clone()),
        interval: Duration::from_millis(25),
    };
    vault.start_indexing(false, true, clock).await.unwrap();
    until(async || row(&vault, "Repeat.md").await["effectiveForDate"] == "2026-08-20").await;
    let first = row(&vault, "Repeat.md").await;
    *date.lock().unwrap() = "2026-08-21".into();
    until(async || row(&vault, "Repeat.md").await["effectiveForDate"] == "2026-08-21").await;
    let next = row(&vault, "Repeat.md").await;
    assert_ne!(
        first["effectiveOccurrenceDate"],
        next["effectiveOccurrenceDate"]
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("Repeat.md")).unwrap(),
        raw
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
}
