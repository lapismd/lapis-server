use lapis_app_core::sync::{ArrivalCursor, ArrivalKind, ArrivalRepository, CatchUpOutcome};
use std::{fs, io::ErrorKind};

fn id(byte: char) -> String {
    std::iter::repeat_n(byte, 64).collect()
}

fn page(
    repository: &ArrivalRepository,
    cursor: Option<&ArrivalCursor>,
    limit: usize,
) -> lapis_app_core::sync::ArrivalPage {
    match repository.page(cursor, limit).unwrap() {
        CatchUpOutcome::Page(page) => page,
        CatchUpOutcome::RestartRequired => panic!("unexpected expired cursor"),
    }
}

#[test]
fn stable_pages_use_arrival_order_and_exclude_later_records() {
    let profile = tempfile::tempdir().unwrap();
    let repository = ArrivalRepository::open(profile.path(), "vault", "server").unwrap();
    repository
        .record_event(ArrivalKind::Revision, &id('f'))
        .unwrap();
    repository
        .record_event(ArrivalKind::Head, &id('1'))
        .unwrap();

    let first = page(&repository, None, 1);
    assert_eq!(first.items[0].event_id, id('f'));
    assert!(!first.complete);
    repository
        .record_event(ArrivalKind::Revision, &id('0'))
        .unwrap();

    let second = page(&repository, first.next_cursor.as_ref(), 8);
    assert_eq!(
        second
            .items
            .iter()
            .map(|item| &item.event_id)
            .collect::<Vec<_>>(),
        [&id('1')]
    );
    assert!(second.complete);
    let fresh = page(&repository, None, 8);
    assert_eq!(
        fresh
            .items
            .iter()
            .map(|item| item.sequence)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
}

#[test]
fn membership_history_is_immutable_and_keeps_exact_payloads() {
    let profile = tempfile::tempdir().unwrap();
    let repository = ArrivalRepository::open(profile.path(), "vault", "server").unwrap();
    let first = repository
        .record_membership(&id('a'), 0, None, b"membership-zero")
        .unwrap();
    let second = repository
        .record_membership(&id('b'), 1, Some(&id('a')), b"membership-one")
        .unwrap();
    assert_eq!(first.item.sequence, 1);
    assert_eq!(second.item.membership_generation, Some(1));
    assert_eq!(
        repository.read_membership(&id('a')).unwrap().unwrap(),
        b"membership-zero"
    );
    assert_eq!(
        repository.read_membership(&id('b')).unwrap().unwrap(),
        b"membership-one"
    );
    assert!(
        repository
            .record_membership(&id('c'), 3, Some(&id('b')), b"gap")
            .is_err()
    );
    assert!(
        repository
            .record_membership(&id('c'), 2, Some(&id('a')), b"stale")
            .is_err()
    );
}

#[test]
fn exact_retries_reuse_one_sequence_and_mismatches_fail_closed() {
    let profile = tempfile::tempdir().unwrap();
    let repository = ArrivalRepository::open(profile.path(), "vault", "server").unwrap();
    let first = repository
        .record_event(ArrivalKind::Revision, &id('a'))
        .unwrap();
    let retry = repository
        .record_event(ArrivalKind::Revision, &id('a'))
        .unwrap();
    assert!(!first.reused);
    assert!(retry.reused);
    assert_eq!(first.item.sequence, retry.item.sequence);
    assert!(
        repository
            .record_event(ArrivalKind::Head, &id('a'))
            .is_err()
    );
    repository
        .record_membership(&id('b'), 0, None, b"sealed")
        .unwrap();
    assert!(
        repository
            .record_membership(&id('b'), 0, None, b"changed")
            .is_err()
    );
}

#[test]
fn empty_pages_are_complete_and_cursor_expiry_requires_restart() {
    let profile = tempfile::tempdir().unwrap();
    let repository = ArrivalRepository::open(profile.path(), "vault", "server").unwrap();
    let empty = page(&repository, None, 4);
    assert!(empty.items.is_empty());
    assert!(empty.complete);
    repository
        .record_event(ArrivalKind::Head, &id('a'))
        .unwrap();
    let cursor = page(&repository, None, 1).cursor;
    repository.expire_cursors_for_rebuild().unwrap();
    assert_eq!(
        repository.page(Some(&cursor), 1).unwrap(),
        CatchUpOutcome::RestartRequired
    );
    assert_eq!(page(&repository, None, 4).items.len(), 1);
}

#[test]
fn cold_reopen_preserves_history_and_exclusive_writer_ownership() {
    let profile = tempfile::tempdir().unwrap();
    let repository = ArrivalRepository::open(profile.path(), "vault", "server").unwrap();
    repository
        .record_event(ArrivalKind::Revision, &id('a'))
        .unwrap();
    let error = match ArrivalRepository::open(profile.path(), "vault", "server") {
        Ok(_) => panic!("a second writer must not open"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
    drop(repository);
    let reopened = ArrivalRepository::open(profile.path(), "vault", "server").unwrap();
    assert_eq!(page(&reopened, None, 8).items[0].event_id, id('a'));
}

#[test]
fn cold_reopen_repairs_an_interrupted_arrival_publication() {
    let profile = tempfile::tempdir().unwrap();
    let event_id = id('a');
    let repository = ArrivalRepository::open(profile.path(), "vault", "server").unwrap();
    repository
        .record_event(ArrivalKind::Revision, &event_id)
        .unwrap();
    drop(repository);

    let destination = profile
        .path()
        .join("sync/v1/vaults/vault/destinations/server");
    let state_path = destination.join("arrivals/state.json");
    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(&state_path).unwrap()).unwrap();
    state["lastSequence"] = 0.into();
    fs::write(&state_path, serde_json::to_vec(&state).unwrap()).unwrap();
    fs::remove_file(
        destination
            .join("arrival-ids")
            .join(format!("{event_id}.json")),
    )
    .unwrap();

    let reopened = ArrivalRepository::open(profile.path(), "vault", "server").unwrap();
    let recovered = page(&reopened, None, 8);
    assert_eq!(recovered.items.len(), 1);
    assert_eq!(recovered.items[0].event_id, event_id);
    assert!(
        reopened
            .record_event(ArrivalKind::Revision, &recovered.items[0].event_id)
            .unwrap()
            .reused
    );
}

#[test]
fn missing_or_corrupt_records_never_become_an_empty_side() {
    let profile = tempfile::tempdir().unwrap();
    let repository = ArrivalRepository::open(profile.path(), "vault", "server").unwrap();
    repository
        .record_event(ArrivalKind::Revision, &id('a'))
        .unwrap();
    fs::remove_file(
        profile
            .path()
            .join("sync/v1/vaults/vault/destinations/server/arrivals/00000000000000000001.json"),
    )
    .unwrap();
    assert_eq!(
        repository.page(None, 8).unwrap_err().kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn replaced_profiles_and_invalid_bounds_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let profile = root.path().join("profile");
    fs::create_dir(&profile).unwrap();
    let repository = ArrivalRepository::open(&profile, "vault", "server").unwrap();
    assert!(repository.page(None, 0).is_err());
    assert!(repository.page(None, 257).is_err());
    fs::rename(&profile, root.path().join("old-profile")).unwrap();
    fs::create_dir(&profile).unwrap();
    assert!(
        repository
            .record_event(ArrivalKind::Head, &id('a'))
            .is_err()
    );
}
