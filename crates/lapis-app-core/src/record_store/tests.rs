use super::{RecordStore, StoredRecord};
use crate::vault::VaultRoot;
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    id: Uuid,
    vault: Uuid,
    text: String,
}
impl StoredRecord for Record {
    const KIND: &'static str = "fixture";
    const MAX_RECORD_BYTES: usize = 512;
    const MAX_RECORDS: usize = 4;
    fn id(&self) -> Uuid {
        self.id
    }
    fn validate(&self, vault: Uuid, id: Uuid) -> io::Result<()> {
        if self.id != id || self.vault != vault || id.get_version_num() != 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid fixture record",
            ));
        }
        Ok(())
    }
}
struct Fixture {
    _temp: tempfile::TempDir,
    profile: PathBuf,
    content: VaultRoot,
    vault: Uuid,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let profile = root.join("profile");
        let content = root.join("vault");
        fs::create_dir(&profile).unwrap();
        fs::create_dir(&content).unwrap();
        Self {
            _temp: temp,
            profile,
            content: VaultRoot::open(&content).unwrap(),
            vault: Uuid::new_v4(),
        }
    }
    fn open(&self) -> RecordStore<Record> {
        RecordStore::open(&self.profile, self.vault, &self.content).unwrap()
    }
    fn record(&self) -> Record {
        Record {
            id: Uuid::new_v4(),
            vault: self.vault,
            text: "Pinned bytes 📝".into(),
        }
    }
    fn path(&self, id: Uuid) -> PathBuf {
        self.profile
            .join(self.vault.to_string())
            .join(format!("{id}.json"))
    }
}

#[test]
fn typed_records_keep_exact_bytes_and_refuse_stale_conditional_publication() {
    let fixture = Fixture::new();
    let store = fixture.open();
    let mut record = fixture.record();
    let first = store.write(&record, None).unwrap();
    assert_eq!(first, serde_json::to_vec(&record).unwrap());
    assert_eq!(
        store.read(record.id).unwrap(),
        Some((record.clone(), first.clone()))
    );
    assert!(store.write(&record, None).is_err());
    record.text = "Next pinned bytes".into();
    let next = store.write(&record, Some(&first)).unwrap();
    assert!(store.write(&record, Some(&first)).is_err());
    assert_eq!(fs::read(fixture.path(record.id)).unwrap(), next);
    assert_eq!(store.ids().unwrap(), vec![record.id]);
}

#[test]
fn independent_store_reuses_exclusive_writer_retirement_and_profile_scope() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    assert!(
        RecordStore::<Record>::open(&fixture.profile, fixture.vault, &fixture.content).is_err()
    );
    let other =
        RecordStore::<Record>::open(&fixture.profile, Uuid::new_v4(), &fixture.content).unwrap();
    drop(other);
    let generation = store.generation;
    store.retire().unwrap();
    assert!(store.ids().is_err());
    let reopened = fixture.open();
    assert_ne!(generation, reopened.generation);
    assert!(store.write(&fixture.record(), None).is_err());
    assert!(RecordStore::<Record>::open(&fixture.profile, Uuid::nil(), &fixture.content).is_err());
    let nested = fixture.content.path().join("private");
    fs::create_dir(&nested).unwrap();
    assert!(RecordStore::<Record>::open(&nested, fixture.vault, &fixture.content).is_err());
}

#[test]
fn typed_validation_and_read_bounds_never_replace_previous_record_bytes() {
    let fixture = Fixture::new();
    let store = fixture.open();
    let record = fixture.record();
    let original = store.write(&record, None).unwrap();
    let mut foreign = record.clone();
    foreign.vault = Uuid::new_v4();
    assert!(store.write(&foreign, Some(&original)).is_err());
    let mut oversized = record.clone();
    oversized.text = "x".repeat(Record::MAX_RECORD_BYTES);
    assert!(store.write(&oversized, Some(&original)).is_err());
    assert_eq!(fs::read(fixture.path(record.id)).unwrap(), original);
    fs::write(
        fixture.path(record.id),
        vec![b' '; Record::MAX_RECORD_BYTES + 1],
    )
    .unwrap();
    assert!(store.read(record.id).is_err());
    fs::write(
        fixture.path(record.id),
        serde_json::to_vec(&foreign).unwrap(),
    )
    .unwrap();
    assert!(store.read(record.id).is_err());
}

#[test]
fn catalogue_keeps_v4_regular_file_and_bounded_enumeration_checks() {
    let fixture = Fixture::new();
    let store = fixture.open();
    for _ in 0..Record::MAX_RECORDS {
        store.write(&fixture.record(), None).unwrap();
    }
    assert_eq!(store.ids().unwrap().len(), Record::MAX_RECORDS);
    let extra = fixture.record();
    fs::write(fixture.path(extra.id), serde_json::to_vec(&extra).unwrap()).unwrap();
    assert!(store.ids().is_err());
    fs::remove_file(fixture.path(extra.id)).unwrap();
    for name in ["00000000-0000-0000-0000-000000000000.json", "invalid.json"] {
        let path = fixture.profile.join(fixture.vault.to_string()).join(name);
        fs::write(&path, "{}").unwrap();
        assert!(store.ids().is_err());
        fs::remove_file(path).unwrap();
    }
    fs::create_dir(fixture.path(Uuid::new_v4())).unwrap();
    assert!(store.ids().is_err());
}

#[test]
fn replaced_private_profile_or_owner_lock_retires_held_authority() {
    for profile in [false, true] {
        let fixture = Fixture::new();
        let store = fixture.open();
        let record = fixture.record();
        store.write(&record, None).unwrap();
        if profile {
            fs::rename(&fixture.profile, fixture.profile.with_extension("old")).unwrap();
            fs::create_dir(&fixture.profile).unwrap();
        } else {
            let lock = fixture
                .profile
                .join(fixture.vault.to_string())
                .join(".owner.lock");
            fs::rename(&lock, lock.with_extension("old")).unwrap();
            fs::write(&lock, "Replacement lock").unwrap();
        }
        assert!(store.read(record.id).is_err());
        assert!(store.write(&record, None).is_err());
    }
}
