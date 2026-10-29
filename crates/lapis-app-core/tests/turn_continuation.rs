use lapis_app_core::{
    turns::{Admission, TurnJournal, TurnOutcome},
    vault::VaultRoot,
};
use lapis_app_protocol::turns::{TurnInput, TurnPhase, TurnRecord};
use serde_json::json;
use std::{fs, path::PathBuf};
use uuid::Uuid;

struct Fixture {
    _temp: tempfile::TempDir,
    profile: PathBuf,
    root: VaultRoot,
    vault: Uuid,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        fs::create_dir(base.join("vault")).unwrap();
        fs::create_dir(base.join("turns")).unwrap();
        Self {
            profile: base.join("turns"),
            root: VaultRoot::open(&base.join("vault")).unwrap(),
            vault: Uuid::new_v4(),
            _temp: temp,
        }
    }
    fn open(&self) -> TurnJournal {
        TurnJournal::open(&self.profile, self.vault, &self.root).unwrap()
    }
}
fn input() -> TurnInput {
    serde_json::from_value(json!({
        "request_id":Uuid::new_v4(), "target_id":"fixture", "scope_dir":"",
        "prompt":"A fresh authored prompt", "context":null
    }))
    .unwrap()
}
fn next(parent: &TurnRecord) -> TurnInput {
    let mut value = serde_json::to_value(input()).unwrap();
    value["continuation"] = json!({
        "conversation_id":parent.conversation_id,
        "after_request_id":parent.input.request_id
    });
    serde_json::from_value(value).unwrap()
}
fn complete(journal: &mut TurnJournal, input: TurnInput) -> TurnRecord {
    let Admission::Fresh { claim, .. } = journal.accept(input).unwrap() else {
        panic!("Expected a fresh native dispatch claim")
    };
    let mut running = journal.claim(claim).unwrap();
    journal
        .finish(
            &mut running,
            TurnOutcome::Completed("Retained response".into()),
        )
        .unwrap()
}

#[test]
fn sequential_turns_keep_one_conversation_and_refuse_stale_branches() {
    let fixture = Fixture::new();
    let mut journal = fixture.open();
    let first = complete(&mut journal, input());
    let followup = next(&first);
    let second = complete(&mut journal, followup.clone());
    assert_eq!(second.conversation_id, first.conversation_id);
    assert!(second.accepted_at_ms > first.updated_at_ms);
    assert!(journal.accept(next(&first)).is_err());
    let Admission::Existing(duplicate) = journal.accept(followup).unwrap() else {
        panic!("A duplicate must not produce dispatch authority")
    };
    assert_eq!(duplicate, second);
    let third = complete(&mut journal, next(&second));
    assert_eq!(third.conversation_id, first.conversation_id);
    assert_eq!(journal.list().unwrap().len(), 3);
}

#[test]
fn continuation_rejects_unfinished_foreign_and_differently_scoped_predecessors() {
    let fixture = Fixture::new();
    let mut journal = fixture.open();
    let Admission::Fresh { record, claim } = journal.accept(input()).unwrap() else {
        panic!("Expected first acceptance")
    };
    assert!(journal.accept(next(&record)).is_err());
    let mut running = journal.claim(claim).unwrap();
    assert!(journal.accept(next(&record)).is_err());
    let first = journal
        .finish(&mut running, TurnOutcome::Completed("Done".into()))
        .unwrap();
    let mut scoped = next(&first);
    scoped.scope_dir = "Other".into();
    assert!(journal.accept(scoped).is_err());
    let mut foreign = first.clone();
    foreign.conversation_id = Uuid::new_v4();
    assert!(journal.accept(next(&foreign)).is_err());
    foreign = first;
    foreign.input.request_id = Uuid::new_v4();
    assert!(journal.accept(next(&foreign)).is_err());
    assert_eq!(journal.list().unwrap().len(), 1);
}

#[test]
fn cold_recovery_retains_the_accepted_head_without_replaying_or_rebranching() {
    let fixture = Fixture::new();
    let mut journal = fixture.open();
    let first = complete(&mut journal, input());
    let followup = next(&first);
    let Admission::Fresh { record, .. } = journal.accept(followup.clone()).unwrap() else {
        panic!("Expected an explicit follow-up")
    };
    drop(journal);
    let mut journal = fixture.open();
    assert!(journal.accept(next(&first)).is_err());
    let Admission::Existing(interrupted) = journal.accept(followup).unwrap() else {
        panic!("Recovery cannot create another dispatch claim")
    };
    assert_eq!(interrupted.phase, TurnPhase::Interrupted);
    assert_eq!(interrupted.conversation_id, first.conversation_id);
    assert_eq!(interrupted.input.request_id, record.input.request_id);
    let fresh = complete(&mut journal, next(&interrupted));
    assert_eq!(fresh.conversation_id, first.conversation_id);
}

#[test]
fn retained_future_timestamps_keep_new_continuations_strictly_ordered() {
    let fixture = Fixture::new();
    let mut journal = fixture.open();
    let mut first = complete(&mut journal, input());
    drop(journal);
    // Simulate a record saved before the installation's clock moved backwards.
    first.accepted_at_ms += 60_000;
    first.updated_at_ms += 60_000;
    let path = fixture
        .profile
        .join(fixture.vault.to_string())
        .join(format!("{}.json", first.input.request_id));
    fs::write(path, serde_json::to_vec(&first).unwrap()).unwrap();
    let mut journal = fixture.open();
    let second = complete(&mut journal, next(&first));
    assert!(second.accepted_at_ms > first.updated_at_ms);
    assert!(second.updated_at_ms >= second.accepted_at_ms);
    assert!(journal.accept(next(&first)).is_err());
}
