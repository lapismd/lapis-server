use super::fixtures::Fixture;
use super::{Checkpoint, TaskLifecycleDisposition, TaskLifecycleJournal};
use lapis_task_services::TaskLifecycleAction;
use std::{fs, sync::mpsc, thread, time::Duration};

#[test]
fn completed_duplicate_observes_its_pinned_result_before_original_version_preflight() {
    let fixture = Fixture::new();
    let vault = fixture.vault();
    let request = fixture.request(&vault);
    let mut journal = TaskLifecycleJournal::open(&fixture.profile, &vault).unwrap();
    let first = journal
        .submit(
            vault.begin_work().unwrap(),
            request.clone(),
            &fixture.policy(),
        )
        .unwrap();
    assert_eq!(first.disposition, TaskLifecycleDisposition::Completed);
    assert_eq!(
        fixture.task(),
        fixture.case["expected"]["attempts"][0]["taskRaw"]
    );
    let task = fixture.task();
    let daily = fixture.daily();
    let observed = journal
        .submit(
            vault.begin_work().unwrap(),
            request.clone(),
            &fixture.policy(),
        )
        .unwrap();
    assert_eq!(observed.disposition, TaskLifecycleDisposition::Completed);
    assert_eq!(fixture.task(), task);
    assert_eq!(fixture.daily(), daily);
    let mut different = request;
    different.action = TaskLifecycleAction::Skip {
        at: "2026-08-20T10:00:00.000Z".into(),
        local_date: "2026-08-20".into(),
    };
    assert!(
        journal
            .submit(vault.begin_work().unwrap(), different, &fixture.policy())
            .is_err()
    );
    assert_eq!(fixture.task(), task);
    assert_eq!(fixture.daily(), daily);
}

#[test]
fn every_uncertain_stage_is_inspected_without_replay_then_explicitly_retried() {
    for stop in [
        Checkpoint::Accepted,
        Checkpoint::Ensuring,
        Checkpoint::Ensured,
        Checkpoint::Observing,
        Checkpoint::Observed,
        Checkpoint::Advancing,
        Checkpoint::Advanced,
    ] {
        let fixture = Fixture::new();
        let vault = fixture.vault();
        let request = fixture.request(&vault);
        let mut journal = TaskLifecycleJournal::open(&fixture.profile, &vault).unwrap();
        assert!(
            journal
                .submit_with(
                    vault.begin_work().unwrap(),
                    request.clone(),
                    &fixture.policy(),
                    |stage| {
                        if stage == stop {
                            Err(std::io::Error::other("simulated owner loss"))
                        } else {
                            Ok(())
                        }
                    }
                )
                .is_err()
        );
        let task = fixture.task();
        let daily = fs::read(fixture.root.join("daily/2026-08-20.md")).ok();
        drop(journal);
        vault.close().unwrap();
        drop(vault);
        let reopened = fixture.vault();
        let mut journal = TaskLifecycleJournal::open(&fixture.profile, &reopened).unwrap();
        let status = journal
            .inspect(&reopened.begin_work().unwrap(), request.request_id)
            .unwrap()
            .unwrap();
        assert_ne!(status.disposition, TaskLifecycleDisposition::Divergent);
        assert_eq!(fixture.task(), task);
        assert_eq!(
            fs::read(fixture.root.join("daily/2026-08-20.md")).ok(),
            daily
        );
        let mut changed_policy = fixture.policy();
        changed_policy.daily.folder = "new-folder".into();
        journal
            .submit(
                reopened.begin_work().unwrap(),
                request.clone(),
                &changed_policy,
            )
            .unwrap();
        assert_eq!(fixture.task(), task);
        assert_eq!(
            fs::read(fixture.root.join("daily/2026-08-20.md")).ok(),
            daily
        );
        let completed = journal
            .retry(reopened.begin_work().unwrap(), request.request_id)
            .unwrap();
        assert_eq!(completed.disposition, TaskLifecycleDisposition::Completed);
        assert_eq!(
            fixture.task(),
            fixture.case["expected"]["attempts"][0]["taskRaw"]
        );
        assert_eq!(
            fixture.daily(),
            fixture.case["expected"]["attempts"][0]["daily"]["daily/2026-08-20.md"]
        );
        assert!(!fixture.root.join("new-folder").exists());
    }
}

#[test]
fn handwritten_original_or_daily_content_survives_partial_and_cold_retry() {
    for path in ["tasks/Read.md", "daily/2026-08-20.md"] {
        let fixture = Fixture::new();
        let vault = fixture.vault();
        let request = fixture.request(&vault);
        let mut journal = TaskLifecycleJournal::open(&fixture.profile, &vault).unwrap();
        assert!(
            journal
                .submit_with(
                    vault.begin_work().unwrap(),
                    request.clone(),
                    &fixture.policy(),
                    |stage| {
                        if stage == Checkpoint::Observed {
                            fs::write(fixture.root.join(path), "Handwritten replacement").unwrap();
                            Err(std::io::Error::other("owner lost"))
                        } else {
                            Ok(())
                        }
                    }
                )
                .is_err()
        );
        drop(journal);
        vault.close().unwrap();
        drop(vault);
        let reopened = fixture.vault();
        let mut journal = TaskLifecycleJournal::open(&fixture.profile, &reopened).unwrap();
        let status = journal
            .inspect(&reopened.begin_work().unwrap(), request.request_id)
            .unwrap()
            .unwrap();
        assert_eq!(status.disposition, TaskLifecycleDisposition::Divergent);
        assert!(
            journal
                .retry(reopened.begin_work().unwrap(), request.request_id)
                .is_err()
        );
        assert_eq!(
            fs::read_to_string(fixture.root.join(path)).unwrap(),
            "Handwritten replacement"
        );
        if path.starts_with("tasks") {
            assert!(fixture.daily().contains("task-occurrence"));
        } else {
            assert_eq!(fixture.task(), fixture.case["raw"]);
        }
    }
}

#[test]
fn an_already_admitted_job_finishes_after_detach_while_close_drains() {
    let fixture = Fixture::new();
    let vault = fixture.vault();
    let request = fixture.request(&vault);
    let client = vault.attach().unwrap();
    let work = vault.begin_work().unwrap();
    let mut journal = TaskLifecycleJournal::open(&fixture.profile, &vault).unwrap();
    drop(client);
    let (accepted_tx, accepted_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let policy = fixture.policy();
    thread::scope(|scope| {
        let job = scope.spawn(move || {
            journal.submit_with(work, request, &policy, |stage| {
                if stage == Checkpoint::Accepted {
                    accepted_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                }
                Ok(())
            })
        });
        accepted_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let close = scope.spawn(|| vault.close());
        for _ in 0..200 {
            if vault.status().unwrap().phase != lapis_app_protocol::VaultPhase::Open {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(vault.begin_work().is_err());
        release_tx.send(()).unwrap();
        assert_eq!(
            job.join().unwrap().unwrap().disposition,
            TaskLifecycleDisposition::Completed
        );
        close.join().unwrap().unwrap();
    });
    assert_eq!(
        fixture.task(),
        fixture.case["expected"]["attempts"][0]["taskRaw"]
    );
}
