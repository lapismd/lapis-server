use super::{support::*, tracked};
use crate::services::{TrackedTaskProvider, daily::DailyFormatContext, indexing::owner::Clock};
use chrono::{DateTime, Local, Utc};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

#[tokio::test]
async fn operator_zone_midnight_and_dst_drive_native_daily_observations() {
    const CHILD: &str = "LAPIS_TASK_ZONE_TEST_CHILD";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let name = format!(
            "{}::operator_zone_midnight_and_dst_drive_native_daily_observations",
            module_path!().split_once("::").unwrap().1
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([name.as_str(), "--exact", "--nocapture"])
            .env(CHILD, "1")
            .env("TZ", "Europe/London")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    assert_eq!(std::env::var("TZ").unwrap(), "Europe/London");
    let cases = [
        ("2026-03-28T23:59:00Z", "2026-03-28", 0),
        ("2026-03-29T00:01:00Z", "2026-03-29", 0),
        ("2026-03-29T00:59:00Z", "2026-03-29", 0),
        ("2026-03-29T01:01:00Z", "2026-03-29", 3600),
        ("2026-03-29T22:59:00Z", "2026-03-29", 3600),
        ("2026-03-29T23:01:00Z", "2026-03-30", 3600),
        ("2026-10-24T22:59:00Z", "2026-10-24", 3600),
        ("2026-10-24T23:01:00Z", "2026-10-25", 3600),
        ("2026-10-25T00:59:00Z", "2026-10-25", 3600),
        ("2026-10-25T01:01:00Z", "2026-10-25", 0),
        ("2026-10-25T23:59:00Z", "2026-10-25", 0),
        ("2026-10-26T00:01:00Z", "2026-10-26", 0),
    ];
    let parse = |value: &str| {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    };
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let raw = tracked::raw().replace("2026-08-20", "2026-03-28");
    write(&root, "Habit.md", &raw);
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let instant = Arc::new(Mutex::new(parse(cases[0].0)));
    let current = instant.clone();
    vault
        .start_indexing(
            false,
            true,
            Clock {
                today: Arc::new(move || {
                    current
                        .lock()
                        .unwrap()
                        .with_timezone(&Local)
                        .format("%Y-%m-%d")
                        .to_string()
                }),
                interval: Duration::from_millis(25),
            },
        )
        .await
        .unwrap();
    let mut policy = tracked::policy();
    policy.format = DailyFormatContext {
        locale: "en-GB".into(),
        first_day: 1,
        minimal_days: 4,
    };
    // Select once. Clock changes below have no client or configuration signal.
    vault
        .set_tracked_task_materialization(Some(TrackedTaskProvider::Default(policy)))
        .await
        .unwrap();
    let mut daily = HashMap::new();
    for (utc, expected, offset) in cases {
        let value = parse(utc);
        let local = value.with_timezone(&Local);
        assert_eq!(local.format("%Y-%m-%d").to_string(), expected);
        assert_eq!(local.offset().local_minus_utc(), offset);
        *instant.lock().unwrap() = value;
        let before = vault.indexing_status().unwrap().passes;
        until(async || {
            let status = vault.indexing_status().unwrap();
            status.passes > before
                && status
                    .tracked
                    .is_some_and(|tracked| tracked.date == expected)
        })
        .await;
        let status = vault.indexing_status().unwrap().tracked.unwrap();
        assert!(status.failures.is_empty(), "{status:?}");
        let text =
            std::fs::read_to_string(root.path().join(format!("daily/{expected}.md"))).unwrap();
        assert!(text.contains(&format!("task=tracked date={expected} kind=boolean")));
        assert_eq!(text.matches("task-occurrence task=tracked").count(), 1);
        if let Some(previous) = daily.insert(expected, text.clone()) {
            assert_eq!(previous, text);
        }
        assert_eq!(
            std::fs::read_to_string(root.path().join("Habit.md")).unwrap(),
            raw
        );
        assert_eq!(vault.status().unwrap().clients, 0);
    }
    close(runtime).await;
}
