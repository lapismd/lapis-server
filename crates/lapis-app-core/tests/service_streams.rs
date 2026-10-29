use lapis_app_core::{
    services::Runtime,
    vault::{MAX_FILE_BYTES, WriteOptions},
};
use lapis_app_protocol::{ErrorCode, ExpectedVersion};
use sha2::{Digest, Sha256};
use std::io::{self, Cursor, Read};
use uuid::Uuid;

fn version(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn byte_streams_keep_binary_versions_snapshots_and_mtime() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    let bytes = [0, 255, 128, 42];
    let hash = client
        .write_from(
            "data.bin",
            bytes.as_slice(),
            Some(&ExpectedVersion::Missing {}),
            &WriteOptions {
                mtime: Some(1700000000000.0),
            },
        )
        .unwrap();
    assert_eq!(hash, version(&bytes));
    let mut snapshot = client.snapshot("data.bin").unwrap();
    assert_eq!(snapshot.length, 4);
    assert_eq!(snapshot.version, hash);
    assert_eq!(
        std::fs::metadata(root.path().join("data.bin"))
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
        1700000000000
    );
    std::fs::write(root.path().join("data.bin"), "external").unwrap();
    let mut captured = Vec::new();
    snapshot.file.read_to_end(&mut captured).unwrap();
    assert_eq!(captured, bytes);
    assert_eq!(
        client
            .write_from(
                "data.bin",
                b"unsafe".as_slice(),
                Some(&ExpectedVersion::Version { version: hash }),
                &WriteOptions::default()
            )
            .unwrap_err()
            .code(),
        ErrorCode::Conflict
    );
    assert_eq!(
        std::fs::read(root.path().join("data.bin")).unwrap(),
        b"external"
    );
}

struct Racing {
    root: std::path::PathBuf,
    bytes: Cursor<&'static [u8]>,
}
impl Read for Racing {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        std::fs::write(self.root.join("note.md"), "external")?;
        self.bytes.read(buffer)
    }
}
#[test]
fn streamed_publication_rechecks_external_bytes_and_dirty_alias_claims() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("note.md"), "old").unwrap();
    std::fs::hard_link(root.path().join("note.md"), root.path().join("alias.md")).unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let a = vault.attach().unwrap();
    let b = vault.attach().unwrap();
    let editor = a.open_editor("note.md").unwrap();
    a.mark_dirty(editor, 1).unwrap();
    assert_eq!(
        b.write_from(
            "alias.md",
            b"unsafe".as_slice(),
            None,
            &WriteOptions::default()
        )
        .unwrap_err()
        .code(),
        ErrorCode::Busy
    );
    drop(a);
    assert_eq!(
        b.write_from(
            "note.md",
            b"unsafe".as_slice(),
            None,
            &WriteOptions::default()
        )
        .unwrap_err()
        .code(),
        ErrorCode::Busy
    );
    b.recover_editor(editor).unwrap();
    b.discard_editor(editor).unwrap();
    let reader = Racing {
        root: root.path().into(),
        bytes: Cursor::new(b"staged"),
    };
    assert_eq!(
        b.write_from(
            "note.md",
            reader,
            Some(&ExpectedVersion::Version {
                version: version(b"old")
            }),
            &WriteOptions::default()
        )
        .unwrap_err()
        .code(),
        ErrorCode::Conflict
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("note.md")).unwrap(),
        "external"
    );
    assert!(!root.path().read_dir().unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("lapis-stage")
    }));
}

struct Broken(bool);
impl Read for Broken {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.0 {
            return Err(io::Error::other("stream interrupted"));
        }
        self.0 = true;
        buffer[0] = b'x';
        Ok(1)
    }
}
#[test]
fn interrupted_and_oversized_sources_never_replace_existing_bytes() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("note.md"), "original").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    assert_eq!(
        client
            .write_from("note.md", Broken(false), None, &WriteOptions::default())
            .unwrap_err()
            .code(),
        ErrorCode::Io
    );
    assert_eq!(
        client
            .write_from(
                "note.md",
                io::repeat(b'x').take(MAX_FILE_BYTES + 1),
                None,
                &WriteOptions::default()
            )
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("note.md")).unwrap(),
        "original"
    );
    let mut oversized = std::fs::File::create(root.path().join("large.bin")).unwrap();
    std::io::Write::write_all(&mut oversized, b"x").unwrap();
    oversized.set_len(MAX_FILE_BYTES + 1).unwrap();
    assert_eq!(
        client.snapshot("large.bin").unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
}

#[test]
fn streamed_reads_and_writes_preserve_physical_admission() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("note.md"), "original").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    assert_eq!(
        client.snapshot("../outside").unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    std::os::unix::fs::symlink("note.md", root.path().join("alias.md")).unwrap();
    assert_eq!(
        client.snapshot("alias.md").unwrap_err().code(),
        ErrorCode::PermissionDenied
    );
    let original = tempfile::tempdir().unwrap();
    std::fs::rename(root.path(), original.path().join("root")).unwrap();
    std::fs::create_dir(root.path()).unwrap();
    assert_eq!(
        client.snapshot("note.md").unwrap_err().code(),
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        client
            .write_from(
                "note.md",
                b"unsafe".as_slice(),
                None,
                &WriteOptions::default()
            )
            .unwrap_err()
            .code(),
        ErrorCode::PermissionDenied
    );
    assert!(!root.path().join("note.md").exists());
}
