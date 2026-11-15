use lapis_app_core::{
    sync::{OperationJournal, OperationKind, OperationPhase},
    vault::VaultRoot,
};
use std::{fs, io};
use uuid::Uuid;

struct Fixture {
    _root: tempfile::TempDir,
    profile: std::path::PathBuf,
    content: VaultRoot,
    vault: Uuid,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let profile = root.path().join("profile");
        let content = root.path().join("vault");
        fs::create_dir(&profile).unwrap();
        fs::create_dir(&content).unwrap();
        Self {
            _root: root,
            profile,
            content: VaultRoot::open(&content).unwrap(),
            vault: Uuid::new_v4(),
        }
    }

    fn open(&self, destination: &str) -> OperationJournal {
        OperationJournal::open(&self.profile, &self.content, self.vault, destination).unwrap()
    }

    fn record_path(&self, destination: &str, id: Uuid) -> std::path::PathBuf {
        self.profile
            .join("sync/v1/vaults")
            .join(self.vault.to_string())
            .join("destinations")
            .join(destination)
            .join("journal")
            .join(format!("{id}.json"))
    }
}

#[test]
fn exact_retry_reuses_ciphertext_without_resetting_accepted_work() {
    let fixture = Fixture::new();
    let journal = fixture.open("server-a");
    let id = Uuid::new_v4();
    let sealed = b"opaque encrypted outbox";

    let staged = journal
        .stage(id, "publish-1", OperationKind::Outbox, sealed)
        .unwrap();
    assert!(!staged.reused);
    assert_eq!(staged.record.phase, OperationPhase::Staged);
    assert_eq!(journal.open_sealed(&staged.record).unwrap(), sealed);

    let accepted = journal
        .transition(id, OperationPhase::Staged, OperationPhase::Accepted)
        .unwrap();
    let retry = journal
        .stage(id, "publish-1", OperationKind::Outbox, sealed)
        .unwrap();
    assert!(retry.reused);
    assert_eq!(retry.record, accepted);

    let mismatch = journal
        .stage(id, "publish-1", OperationKind::Outbox, b"fresh ciphertext")
        .unwrap_err();
    assert_eq!(mismatch.kind(), io::ErrorKind::InvalidData);
    assert_eq!(journal.read(id).unwrap(), Some(accepted));
}

#[test]
fn unknown_outcome_requires_inspection_before_apply_and_survives_reopen() {
    let fixture = Fixture::new();
    let mut journal = fixture.open("server-a");
    let id = Uuid::new_v4();
    journal
        .stage(id, "apply-1", OperationKind::Apply, b"sealed apply intent")
        .unwrap();
    journal
        .transition(id, OperationPhase::Staged, OperationPhase::Unknown)
        .unwrap();
    assert!(
        journal
            .transition(id, OperationPhase::Unknown, OperationPhase::Applied)
            .is_err()
    );
    journal
        .transition(id, OperationPhase::Unknown, OperationPhase::Accepted)
        .unwrap();
    journal
        .transition(id, OperationPhase::Accepted, OperationPhase::Applied)
        .unwrap();
    journal.retire().unwrap();
    assert!(journal.read(id).is_err());

    let reopened = fixture.open("server-a");
    let record = reopened.read(id).unwrap().unwrap();
    assert_eq!(record.phase, OperationPhase::Applied);
    assert_eq!(
        reopened.open_sealed(&record).unwrap(),
        b"sealed apply intent"
    );
}

#[test]
fn stale_transitions_and_foreign_retry_authority_preserve_original_bytes() {
    let fixture = Fixture::new();
    let journal = fixture.open("server-a");
    let id = Uuid::new_v4();
    let first = journal
        .stage(id, "request-a", OperationKind::Outbox, b"ciphertext-a")
        .unwrap()
        .record;
    let bytes = fs::read(fixture.record_path("server-a", id)).unwrap();

    assert!(
        journal
            .transition(id, OperationPhase::Accepted, OperationPhase::Applied)
            .is_err()
    );
    assert!(
        journal
            .stage(id, "request-b", OperationKind::Outbox, b"ciphertext-a")
            .is_err()
    );
    assert!(
        journal
            .stage(id, "request-a", OperationKind::Apply, b"ciphertext-a")
            .is_err()
    );
    assert_eq!(
        fs::read(fixture.record_path("server-a", id)).unwrap(),
        bytes
    );
    assert_eq!(journal.read(id).unwrap(), Some(first));
}

#[test]
fn exclusive_destination_owner_and_physical_root_are_fenced() {
    let fixture = Fixture::new();
    let journal = fixture.open("server-a");
    assert!(
        OperationJournal::open(
            &fixture.profile,
            &fixture.content,
            fixture.vault,
            "server-a"
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(fixture.content.path()).unwrap().count(), 0);
    let other = fixture.open("server-b");
    let other_id = Uuid::new_v4();
    other
        .stage(
            other_id,
            "other",
            OperationKind::Outbox,
            b"other ciphertext",
        )
        .unwrap();
    assert!(journal.read(other_id).unwrap().is_none());

    let lock_owner = fixture.open("server-lock");
    let lock = fixture
        .profile
        .join("sync/v1/vaults")
        .join(fixture.vault.to_string())
        .join("destinations/server-lock/journal/.owner.lock");
    fs::rename(&lock, lock.with_extension("retained")).unwrap();
    fs::write(&lock, b"replacement").unwrap();
    assert_eq!(
        lock_owner.read(Uuid::new_v4()).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );

    let retained = fixture.profile.with_extension("retained");
    fs::rename(&fixture.profile, retained).unwrap();
    fs::create_dir(&fixture.profile).unwrap();
    assert_eq!(
        journal.read(Uuid::new_v4()).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
}

#[test]
fn invalid_roots_corruption_and_payload_bounds_fail_closed() {
    let fixture = Fixture::new();
    assert!(
        OperationJournal::open(
            fixture.content.path(),
            &fixture.content,
            fixture.vault,
            "server-a",
        )
        .is_err()
    );
    assert!(
        OperationJournal::open(
            &fixture.profile,
            &fixture.content,
            fixture.vault,
            "../server"
        )
        .is_err()
    );

    let journal = fixture.open("server-a");
    assert!(
        journal
            .stage(Uuid::new_v4(), "empty", OperationKind::Outbox, b"")
            .is_err()
    );
    assert!(
        journal
            .stage(
                Uuid::new_v4(),
                "large",
                OperationKind::Outbox,
                &vec![0; 512 * 1024 + 1],
            )
            .is_err()
    );

    let id = Uuid::new_v4();
    journal
        .stage(id, "corrupt", OperationKind::Outbox, b"ciphertext")
        .unwrap();
    fs::write(fixture.record_path("server-a", id), b"{}").unwrap();
    assert_eq!(
        journal.read(id).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}
