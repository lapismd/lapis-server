use lapis_app_core::sync::{EventKind, EventRepository};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor};

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            root: tempfile::tempdir().unwrap(),
        }
    }

    fn open(&self) -> EventRepository {
        EventRepository::open(self.root.path(), "vault-1", "server-1").unwrap()
    }

    fn object(&self, bytes: &[u8]) -> String {
        let id = id(bytes);
        self.open()
            .objects()
            .put(&id, &mut Cursor::new(bytes))
            .unwrap();
        id
    }

    fn path(&self, leaf: &str, name: &str) -> std::path::PathBuf {
        self.root
            .path()
            .join("sync/v1/vaults/vault-1/destinations/server-1")
            .join(leaf)
            .join(name)
    }
}

fn id(seed: &[u8]) -> String {
    format!("{:x}", Sha256::digest(seed))
}

#[test]
fn revision_acceptance_requires_durable_objects_and_reuses_exact_event_bytes() {
    let fixture = Fixture::new();
    let repository = fixture.open();
    let event_id = id(b"revision-1");
    let object_id = id(b"missing object");

    let missing = repository
        .accept_revision(
            &event_id,
            b"opaque signed revision",
            std::slice::from_ref(&object_id),
            &[],
        )
        .unwrap_err();
    assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);
    assert!(!fixture.path("events", &format!("{event_id}.json")).exists());

    repository
        .objects()
        .put(&object_id, &mut Cursor::new(b"missing object"))
        .unwrap();
    let first = repository
        .accept_revision(
            &event_id,
            b"opaque signed revision",
            std::slice::from_ref(&object_id),
            &[],
        )
        .unwrap();
    assert!(!first.reused);
    assert_eq!(first.event.kind, EventKind::Revision);

    let retry = repository
        .accept_revision(
            &event_id,
            b"opaque signed revision",
            std::slice::from_ref(&object_id),
            &[],
        )
        .unwrap();
    assert!(retry.reused);
    assert_eq!(retry.event, first.event);
    assert!(
        repository
            .accept_revision(
                &event_id,
                b"different signed bytes",
                std::slice::from_ref(&object_id),
                &[],
            )
            .is_err()
    );
}

#[test]
fn revision_parent_dependencies_must_already_exist() {
    let fixture = Fixture::new();
    let repository = fixture.open();
    let parent = id(b"parent");
    let child = id(b"child");

    assert!(
        repository
            .accept_revision(&child, b"child event", &[], std::slice::from_ref(&parent))
            .is_err()
    );
    repository
        .accept_revision(&parent, b"parent event", &[], &[])
        .unwrap();
    repository
        .accept_revision(&child, b"child event", &[], std::slice::from_ref(&parent))
        .unwrap();
    let restored = repository.read_event(&child).unwrap().unwrap();
    assert_eq!(restored.event_dependencies, vec![parent]);
}

#[test]
fn heads_are_discoverable_only_after_their_revision_graph_is_durable() {
    let fixture = Fixture::new();
    let repository = fixture.open();
    let object = fixture.object(b"encrypted archive");
    let revision = id(b"revision");
    repository
        .accept_revision(
            &revision,
            b"signed revision",
            std::slice::from_ref(&object),
            &[],
        )
        .unwrap();
    let head = id(b"head-a");
    let published = repository
        .publish_head(
            "device-a",
            &head,
            b"signed head",
            std::slice::from_ref(&revision),
            None,
        )
        .unwrap();
    assert!(!published.event_reused);
    assert!(!published.pointer_reused);
    assert_eq!(repository.head("device-a").unwrap().unwrap().event.id, head);
    assert!(repository.head("device-b").unwrap().is_none());

    drop(repository);
    let reopened = fixture.open();
    assert_eq!(
        reopened.head("device-a").unwrap().unwrap().revision_ids,
        vec![revision]
    );
}

#[test]
fn stale_head_cas_leaves_the_new_event_durable_but_not_discoverable() {
    let fixture = Fixture::new();
    let repository = fixture.open();
    let first_revision = id(b"revision-1");
    let second_revision = id(b"revision-2");
    repository
        .accept_revision(&first_revision, b"revision one", &[], &[])
        .unwrap();
    repository
        .accept_revision(&second_revision, b"revision two", &[], &[])
        .unwrap();
    let first_head = id(b"head-1");
    repository
        .publish_head(
            "device-a",
            &first_head,
            b"head one",
            std::slice::from_ref(&first_revision),
            None,
        )
        .unwrap();

    let second_head = id(b"head-2");
    let stale = repository
        .publish_head(
            "device-a",
            &second_head,
            b"head two",
            std::slice::from_ref(&second_revision),
            Some(&id(b"foreign head")),
        )
        .unwrap_err();
    assert_eq!(stale.kind(), std::io::ErrorKind::WouldBlock);
    assert_eq!(
        repository.head("device-a").unwrap().unwrap().event.id,
        first_head
    );
    assert!(repository.read_event(&second_head).unwrap().is_some());

    let accepted = repository
        .publish_head(
            "device-a",
            &second_head,
            b"head two",
            std::slice::from_ref(&second_revision),
            Some(&first_head),
        )
        .unwrap();
    assert!(accepted.event_reused);
    let lost_ack_retry = repository
        .publish_head(
            "device-a",
            &second_head,
            b"head two",
            std::slice::from_ref(&second_revision),
            Some(&first_head),
        )
        .unwrap();
    assert!(lost_ack_retry.pointer_reused);
}

#[test]
fn corrupt_or_missing_transitive_dependencies_fail_closed() {
    let fixture = Fixture::new();
    let repository = fixture.open();
    let object = fixture.object(b"encrypted object");
    let revision = id(b"revision");
    repository
        .accept_revision(&revision, b"revision", std::slice::from_ref(&object), &[])
        .unwrap();
    fs::write(fixture.path("objects", &object), b"corrupt").unwrap();

    assert_eq!(
        repository.read_event(&revision).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    let head = id(b"head");
    assert!(
        repository
            .publish_head(
                "device-a",
                &head,
                b"head",
                std::slice::from_ref(&revision),
                None,
            )
            .is_err()
    );
    assert!(repository.head("device-a").unwrap().is_none());
}

#[test]
fn concurrent_exact_publication_converges_on_one_event_and_head() {
    let fixture = Fixture::new();
    let revision = id(b"concurrent revision");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let root = fixture.root.path().to_owned();
        let revision = revision.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let repository = EventRepository::open(&root, "vault-1", "server-1").unwrap();
            barrier.wait();
            repository
                .accept_revision(&revision, b"same signed revision", &[], &[])
                .unwrap()
                .reused
        }));
    }
    let mut reused: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    reused.sort();
    assert_eq!(reused, vec![false, true]);

    let head = id(b"concurrent head");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let root = fixture.root.path().to_owned();
        let revision = revision.clone();
        let head = head.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let repository = EventRepository::open(&root, "vault-1", "server-1").unwrap();
            barrier.wait();
            repository
                .publish_head("device-a", &head, b"same signed head", &[revision], None)
                .unwrap()
        }));
    }
    let published: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(
        published
            .iter()
            .filter(|result| result.pointer_reused)
            .count(),
        1
    );
    assert_eq!(
        fixture.open().head("device-a").unwrap().unwrap().event.id,
        head
    );
}

#[test]
fn corrupt_records_and_replaced_profiles_never_become_empty_state() {
    let fixture = Fixture::new();
    let repository = fixture.open();
    let revision = id(b"revision");
    repository
        .accept_revision(&revision, b"revision", &[], &[])
        .unwrap();
    fs::write(fixture.path("events", &format!("{revision}.json")), b"{}").unwrap();
    assert_eq!(
        repository.read_event(&revision).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );

    let clean = id(b"clean revision");
    repository
        .accept_revision(&clean, b"clean revision", &[], &[])
        .unwrap();
    let head = id(b"head");
    repository
        .publish_head(
            "device-a",
            &head,
            b"head",
            std::slice::from_ref(&clean),
            None,
        )
        .unwrap();
    fs::write(fixture.path("heads", "device-a.json"), b"{}").unwrap();
    assert_eq!(
        repository.head("device-a").unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );

    let retained = fixture.root.path().with_extension("retained");
    fs::rename(fixture.root.path(), retained).unwrap();
    fs::create_dir(fixture.root.path()).unwrap();
    assert_eq!(
        repository.head("device-a").unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
}

#[test]
fn invalid_authority_and_bounded_inputs_are_refused_before_storage() {
    let fixture = Fixture::new();
    let repository = fixture.open();
    let revision = id(b"revision");
    assert!(
        repository
            .accept_revision(&revision, b"", &[], &[])
            .is_err()
    );
    assert!(
        repository
            .accept_revision(&revision, &vec![0; 512 * 1024 + 1], &[], &[])
            .is_err()
    );
    let duplicate = id(b"duplicate");
    assert!(
        repository
            .accept_revision(&revision, b"revision", &[duplicate.clone(), duplicate], &[],)
            .is_err()
    );
    let too_many: Vec<_> = (0..65)
        .map(|index| id(format!("revision-{index}").as_bytes()))
        .collect();
    assert!(
        repository
            .publish_head("device-a", &id(b"head"), b"head", &too_many, None)
            .is_err()
    );
    assert!(
        repository
            .publish_head("../device", &id(b"head"), b"head", &[revision], None)
            .is_err()
    );
    assert_eq!(fs::read_dir(fixture.path("events", "")).unwrap().count(), 0);
}
