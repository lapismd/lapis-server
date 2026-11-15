use lapis_app_core::sync::{ObjectRepository, TRANSFER_BUFFER_BYTES};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    fs,
    io::{self, Cursor, Read},
};

const _: () = assert!(TRANSFER_BUFFER_BYTES < 64 * 1024 * 1024);

fn object_id(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn repository() -> (tempfile::TempDir, ObjectRepository) {
    let profile = tempfile::tempdir().unwrap();
    let repository = ObjectRepository::open(profile.path(), "vault-1", "server-1").unwrap();
    (profile, repository)
}

#[test]
fn stores_then_reuses_the_same_encrypted_object_identity() {
    let (_profile, repository) = repository();
    let bytes = b"ciphertext remains stable";
    let id = object_id(bytes);

    let first = repository.put(&id, &mut Cursor::new(bytes)).unwrap();
    assert_eq!(first.size, bytes.len() as u64);
    assert!(!first.reused);

    let second = repository.put(&id, &mut Cursor::new(bytes)).unwrap();
    assert_eq!(second, first_with_reuse(first));

    let mut restored = Vec::new();
    assert_eq!(
        repository.copy_verified(&id, &mut restored).unwrap(),
        bytes.len() as u64
    );
    assert_eq!(restored, bytes);
}

#[test]
fn rejects_mismatched_identity_without_publishing_bytes() {
    let (_profile, repository) = repository();
    let expected = object_id(b"expected ciphertext");
    let error = repository
        .put(&expected, &mut Cursor::new(b"different ciphertext"))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

    let mut bytes = Vec::new();
    assert_eq!(
        repository
            .copy_verified(&expected, &mut bytes)
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
}

#[test]
fn refuses_corrupt_existing_bytes_instead_of_replacing_or_serving_them() {
    let (profile, repository) = repository();
    let bytes = b"original ciphertext";
    let id = object_id(bytes);
    repository.put(&id, &mut Cursor::new(bytes)).unwrap();
    let path = profile
        .path()
        .join("sync/v1/vaults/vault-1/destinations/server-1/objects")
        .join(&id);
    fs::write(path, b"corrupt ciphertext").unwrap();

    assert_eq!(
        repository
            .put(&id, &mut Cursor::new(bytes))
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
    let mut target = Vec::new();
    assert_eq!(
        repository
            .copy_verified(&id, &mut target)
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
    assert!(target.is_empty());
}

#[test]
fn binds_storage_to_independent_profile_vault_and_destination_roots() {
    let profile = tempfile::tempdir().unwrap();
    let first = ObjectRepository::open(profile.path(), "vault-a", "server-a").unwrap();
    let second = ObjectRepository::open(profile.path(), "vault-b", "server-a").unwrap();
    let bytes = b"encrypted object";
    let id = object_id(bytes);
    first.put(&id, &mut Cursor::new(bytes)).unwrap();

    assert_eq!(
        second
            .copy_verified(&id, &mut Vec::new())
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
    assert!(ObjectRepository::open(profile.path(), "../vault-a", "server-a").is_err());
    assert!(ObjectRepository::open(profile.path(), "vault-a", "server/a").is_err());
}

#[test]
fn refuses_a_replaced_profile_instead_of_writing_to_a_stale_root() {
    let (profile, repository) = repository();
    let retained = profile.path().with_extension("retained");
    fs::rename(profile.path(), &retained).unwrap();
    fs::create_dir(profile.path()).unwrap();
    let bytes = b"do not publish";
    let error = repository
        .put(&object_id(bytes), &mut Cursor::new(bytes))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[cfg(unix)]
#[test]
fn refuses_symlinked_repository_components_and_objects() {
    use std::os::unix::fs::symlink;

    let profile = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), profile.path().join("sync")).unwrap();
    assert_eq!(
        ObjectRepository::open(profile.path(), "vault-1", "server-1")
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::PermissionDenied
    );

    fs::remove_file(profile.path().join("sync")).unwrap();
    let repository = ObjectRepository::open(profile.path(), "vault-1", "server-1").unwrap();
    let id = object_id(b"outside");
    let object_path = profile
        .path()
        .join("sync/v1/vaults/vault-1/destinations/server-1/objects")
        .join(&id);
    symlink(outside.path(), object_path).unwrap();
    assert_eq!(
        repository
            .copy_verified(&id, &mut Vec::new())
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
}

struct ObservedReader {
    remaining: usize,
    largest_request: Cell<usize>,
}

impl Read for ObservedReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.largest_request
            .set(self.largest_request.get().max(buffer.len()));
        let read = buffer.len().min(self.remaining);
        buffer[..read].fill(0x5a);
        self.remaining -= read;
        Ok(read)
    }
}

#[test]
fn streams_objects_with_a_buffer_below_the_in_flight_target() {
    const SIZE: usize = 2 * 1024 * 1024 + 17;
    let (_profile, repository) = repository();
    let bytes = vec![0x5a; SIZE];
    let id = object_id(&bytes);
    let mut source = ObservedReader {
        remaining: SIZE,
        largest_request: Cell::new(0),
    };

    let stored = repository.put(&id, &mut source).unwrap();
    assert_eq!(stored.size, SIZE as u64);
    assert!(source.largest_request.get() <= TRANSFER_BUFFER_BYTES);
}

fn first_with_reuse(
    mut stored: lapis_app_core::sync::StoredObject,
) -> lapis_app_core::sync::StoredObject {
    stored.reused = true;
    stored
}
