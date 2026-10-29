use lapis_app_core::{
    turns::{Admission, TurnJournal, TurnOutcome},
    vault::VaultRoot,
};
use lapis_app_protocol::turns::{TurnInput, TurnPhase};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use uuid::Uuid;

struct Fixture {
    _temp: tempfile::TempDir,
    profile: PathBuf,
    content: VaultRoot,
    vault: Uuid,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let profile = temp.path().join("profile");
        let content = temp.path().join("vault");
        fs::create_dir(&profile).unwrap();
        fs::create_dir(&content).unwrap();
        Self {
            profile,
            content: VaultRoot::open(&content).unwrap(),
            vault: Uuid::new_v4(),
            _temp: temp,
        }
    }
    fn open(&self) -> TurnJournal {
        TurnJournal::open(&self.profile, self.vault, &self.content).unwrap()
    }
    fn path(&self, id: Uuid) -> PathBuf {
        self.profile
            .join(self.vault.to_string())
            .join(format!("{id}.json"))
    }
}
fn input() -> TurnInput {
    TurnInput {
        request_id: Uuid::new_v4(),
        continuation: None,
        target_id: "codex".into(),
        scope_dir: "./Notes/".into(),
        prompt: "Authored café 📝".into(),
        context: Some("Separate untrusted context".into()),
        options: Default::default(),
    }
}
fn fresh(journal: &mut TurnJournal, input: TurnInput) -> lapis_app_core::turns::DispatchClaim {
    match journal.accept(input).unwrap() {
        Admission::Fresh { claim, .. } => claim,
        Admission::Existing(_) => panic!("fresh admission required"),
    }
}

#[test]
fn duplicate_and_reused_requests_never_create_a_second_dispatch_claim() {
    let f = Fixture::new();
    let mut journal = f.open();
    let request = input();
    let claim = fresh(&mut journal, request.clone());
    let Admission::Existing(saved) = journal.accept(request.clone()).unwrap() else {
        panic!("duplicate must observe")
    };
    assert_eq!(saved.phase, TurnPhase::Accepted);
    assert_eq!(saved.input.scope_dir, "Notes");
    assert_eq!(saved.input.prompt, request.prompt);
    assert_eq!(saved.input.context, request.context);
    let mut changed = request.clone();
    changed.prompt = "different".into();
    assert!(journal.accept(changed).is_err());
    let mut running = journal.claim(claim).unwrap();
    let completed = journal
        .finish(
            &mut running,
            TurnOutcome::Completed("Persisted reply".into()),
        )
        .unwrap();
    assert!(journal.progress(&mut running, "late".into()).is_err());
    assert!(
        journal
            .finish(&mut running, TurnOutcome::Cancelled)
            .is_err()
    );
    drop(journal);
    let mut reopened = f.open();
    assert_eq!(
        reopened.read(request.request_id).unwrap(),
        Some(completed.clone())
    );
    let Admission::Existing(duplicate) = reopened.accept(request).unwrap() else {
        panic!("restart cannot dispatch")
    };
    assert_eq!(duplicate, completed);
}

#[test]
fn restart_retains_interrupted_and_uncertain_states_without_reusable_claims() {
    let f = Fixture::new();
    let mut journal = f.open();
    let accepted = input();
    let claim = fresh(&mut journal, accepted.clone());
    let streaming = input();
    let start = fresh(&mut journal, streaming.clone());
    let mut running = journal.claim(start).unwrap();
    journal.progress(&mut running, "partial".into()).unwrap();
    drop(journal);
    let mut recovered = f.open();
    assert_eq!(
        recovered.read(accepted.request_id).unwrap().unwrap().phase,
        TurnPhase::Interrupted
    );
    let uncertain = recovered.read(streaming.request_id).unwrap().unwrap();
    assert_eq!(uncertain.phase, TurnPhase::Unknown);
    assert_eq!(uncertain.output.as_deref(), Some("partial"));
    assert!(recovered.claim(claim).is_err());
    assert!(recovered.progress(&mut running, "stale".into()).is_err());
    assert!(matches!(
        recovered.accept(accepted).unwrap(),
        Admission::Existing(_)
    ));
    assert!(matches!(
        recovered.accept(streaming).unwrap(),
        Admission::Existing(_)
    ));
}

#[test]
fn live_writer_is_exclusive_and_vault_content_cannot_be_dispatch_authority() {
    let f = Fixture::new();
    let journal = f.open();
    assert!(TurnJournal::open(&f.profile, f.vault, &f.content).is_err());
    let other = TurnJournal::open(&f.profile, Uuid::new_v4(), &f.content).unwrap();
    drop(other);
    drop(journal);
    let nested = f.content.path().join("private");
    fs::create_dir(&nested).unwrap();
    assert!(TurnJournal::open(&nested, f.vault, &f.content).is_err());
    assert!(TurnJournal::open(&f.profile, Uuid::nil(), &f.content).is_err());
    let parent = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let content = parent.path().join(id.to_string());
    fs::create_dir(&content).unwrap();
    let content = VaultRoot::open(&content).unwrap();
    assert!(
        TurnJournal::open(parent.path(), id, &content).is_err(),
        "a private journal cannot resolve inside an admitted vault"
    );
    assert!(!content.path().join(".owner.lock").exists());
}

#[test]
fn changed_records_and_replaced_writer_locks_cannot_be_overwritten() {
    let f = Fixture::new();
    let mut journal = f.open();
    let request = input();
    let claim = fresh(&mut journal, request.clone());
    fs::write(f.path(request.request_id), "external private edit").unwrap();
    assert!(journal.claim(claim).is_err());
    assert_eq!(
        fs::read_to_string(f.path(request.request_id)).unwrap(),
        "external private edit"
    );
    let next = input();
    let claim = fresh(&mut journal, next.clone());
    let mut running = journal.claim(claim).unwrap();
    journal
        .progress(&mut running, "saved partial".into())
        .unwrap();
    fs::write(f.path(next.request_id), "another external edit").unwrap();
    assert!(
        journal
            .finish(&mut running, TurnOutcome::Completed("unsafe".into()))
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(f.path(next.request_id)).unwrap(),
        "another external edit"
    );
    let lock = f.profile.join(f.vault.to_string()).join(".owner.lock");
    fs::rename(&lock, lock.with_extension("retired")).unwrap();
    fs::write(&lock, "").unwrap();
    assert!(journal.list().is_err());
}

#[test]
fn invalid_inputs_and_overflow_preserve_the_last_durable_progress() {
    let f = Fixture::new();
    let mut journal = f.open();
    for scope in [
        "/absolute",
        "//",
        "../escape",
        ".lapis/private",
        "Notes/../other",
        "C:\\outside",
        "./",
    ] {
        let mut invalid = input();
        invalid.scope_dir = scope.into();
        assert!(journal.accept(invalid).is_err(), "{scope}");
    }
    let mut invalid = input();
    invalid.prompt = "x".repeat(1024 * 1024 + 1);
    assert!(journal.accept(invalid).is_err());
    let mut invalid = input();
    invalid.request_id = Uuid::nil();
    assert!(journal.accept(invalid).is_err());
    assert!(journal.list().unwrap().is_empty());
    let request = input();
    let claim = fresh(&mut journal, request.clone());
    let mut running = journal.claim(claim).unwrap();
    journal.progress(&mut running, "retained".into()).unwrap();
    assert!(
        journal
            .progress(&mut running, "x".repeat(4 * 1024 * 1024 + 1))
            .is_err()
    );
    assert_eq!(
        journal
            .read(request.request_id)
            .unwrap()
            .unwrap()
            .output
            .as_deref(),
        Some("retained")
    );
    journal
        .finish(&mut running, TurnOutcome::Unknown("output_limit".into()))
        .unwrap();
}

#[test]
fn malformed_and_foreign_records_fail_closed_without_repairing_user_bytes() {
    let f = Fixture::new();
    let mut journal = f.open();
    let request = input();
    let _claim = fresh(&mut journal, request.clone());
    let path = f.path(request.request_id);
    let mut json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    json["vault_id"] = serde_json::json!(Uuid::new_v4());
    let bytes = serde_json::to_vec(&json).unwrap();
    fs::write(&path, &bytes).unwrap();
    drop(journal);
    assert!(TurnJournal::open(&f.profile, f.vault, &f.content).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn unsupported_or_inconsistent_states_are_not_treated_as_valid_outcomes() {
    for (field, value) in [
        ("schema_version", serde_json::json!(2)),
        ("phase", serde_json::json!("failed")),
        ("phase", serde_json::json!("completed")),
        ("phase", serde_json::json!("unknown")),
        ("output", serde_json::json!("not dispatched")),
        ("updated_at_ms", serde_json::json!(u64::MAX)),
    ] {
        let f = Fixture::new();
        let mut journal = f.open();
        let request = input();
        let _claim = fresh(&mut journal, request.clone());
        let path = f.path(request.request_id);
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        record[field] = value;
        let bytes = serde_json::to_vec(&record).unwrap();
        fs::write(&path, &bytes).unwrap();
        drop(journal);
        assert!(
            TurnJournal::open(&f.profile, f.vault, &f.content).is_err(),
            "{field}"
        );
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

#[cfg(unix)]
#[test]
fn replaced_profile_and_linked_roots_refuse_existing_authority() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let journal = f.open();
    let retired = f.profile.with_extension("retired");
    fs::rename(&f.profile, &retired).unwrap();
    symlink(&retired, &f.profile).unwrap();
    assert!(journal.list().is_err());
    drop(journal);
    assert!(TurnJournal::open(&f.profile, f.vault, &f.content).is_err());
}

#[test]
fn actual_process_loss_cannot_reissue_an_accepted_or_running_dispatch() {
    for phase in ["accepted", "running"] {
        let f = Fixture::new();
        let id = Uuid::new_v4();
        let ready = f.profile.join("peer-ready");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "crash_peer", "--ignored", "--nocapture"])
            .env("LAPIS_TURN_PEER_PROFILE", &f.profile)
            .env("LAPIS_TURN_PEER_CONTENT", f.content.path())
            .env("LAPIS_TURN_PEER_VAULT", f.vault.to_string())
            .env("LAPIS_TURN_PEER_REQUEST", id.to_string())
            .env("LAPIS_TURN_PEER_PHASE", phase)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ready.exists() && Instant::now() < deadline {
            assert!(
                child.try_wait().unwrap().is_none(),
                "peer exited before durability"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        if !ready.exists() {
            let _ = child.kill();
            let _ = child.wait();
            panic!("peer readiness deadline");
        }
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
        let mut journal = f.open();
        let record = journal.read(id).unwrap().unwrap();
        assert_eq!(
            record.phase,
            if phase == "accepted" {
                TurnPhase::Interrupted
            } else {
                TurnPhase::Unknown
            }
        );
        assert!(matches!(
            journal.accept(record.input).unwrap(),
            Admission::Existing(_)
        ));
    }
}

#[test]
#[ignore = "isolated crash-process helper"]
fn crash_peer() {
    let profile = PathBuf::from(std::env::var_os("LAPIS_TURN_PEER_PROFILE").unwrap());
    let content = VaultRoot::open(&PathBuf::from(
        std::env::var_os("LAPIS_TURN_PEER_CONTENT").unwrap(),
    ))
    .unwrap();
    let vault = Uuid::parse_str(&std::env::var("LAPIS_TURN_PEER_VAULT").unwrap()).unwrap();
    let mut journal = TurnJournal::open(&profile, vault, &content).unwrap();
    let mut request = input();
    request.request_id =
        Uuid::parse_str(&std::env::var("LAPIS_TURN_PEER_REQUEST").unwrap()).unwrap();
    let claim = fresh(&mut journal, request);
    let _running = if std::env::var("LAPIS_TURN_PEER_PHASE").unwrap() == "running" {
        Some(journal.claim(claim).unwrap())
    } else {
        None
    };
    fs::write(profile.join("peer-ready"), "durable").unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn full_journal_refuses_new_admission_without_deleting_completed_history() {
    let f = Fixture::new();
    let mut journal = f.open();
    let first = input();
    let claim = fresh(&mut journal, first.clone());
    let mut running = journal.claim(claim).unwrap();
    let saved = journal
        .finish(&mut running, TurnOutcome::Completed("completed".into()))
        .unwrap();
    drop(journal);
    for _ in 1..4096 {
        let mut record = saved.clone();
        record.input.request_id = Uuid::new_v4();
        fs::write(
            f.path(record.input.request_id),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
    }
    let mut journal = f.open();
    let summaries = journal.list().unwrap();
    assert_eq!(summaries.len(), 4096);
    assert!(
        summaries
            .iter()
            .all(|summary| summary.phase == TurnPhase::Completed)
    );
    assert_eq!(
        journal.accept(input()).err().unwrap().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(matches!(
        journal.accept(first).unwrap(),
        Admission::Existing(_)
    ));
    assert_eq!(
        journal.read(saved.input.request_id).unwrap().unwrap(),
        saved
    );
}
