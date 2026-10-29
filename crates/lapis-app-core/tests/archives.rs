use lapis_app_core::{
    archive::{ArchiveLimits, create_zip, extract_zip},
    vault::VaultRoot,
};
use std::io::{Cursor, Read, Seek, SeekFrom};

const NOTES: &[u8] = include_bytes!("fixtures/archives/notes.zip");
const WINDOWS: &[u8] = include_bytes!("fixtures/archives/windows.zip");
const TRAVERSAL: &[u8] = include_bytes!("fixtures/archives/traversal.zip");
const ABSOLUTE: &[u8] = include_bytes!("fixtures/archives/absolute.zip");

#[test]
fn retained_browser_archives_preserve_binary_unicode_and_application_state() {
    let folder = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(folder.path()).unwrap();
    extract_zip(Cursor::new(NOTES), &root, ArchiveLimits::default()).unwrap();
    assert_eq!(root.read("image.bin").unwrap(), vec![0, 128, 255]);
    assert_eq!(
        root.read_text("notes/café.md").unwrap(),
        "# Imported café\n"
    );
    assert_eq!(
        root.read_text(".lapis/app.json").unwrap(),
        "{\"appearance\":\"dark\"}"
    );
    let mut snapshot = create_zip(&root, ArchiveLimits::default()).unwrap();
    assert!(snapshot.length > 22);
    let mut bytes = Vec::new();
    snapshot.file.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes.len() as u64, snapshot.length);
    let second = tempfile::tempdir().unwrap();
    let restored = VaultRoot::open(second.path()).unwrap();
    extract_zip(Cursor::new(bytes), &restored, ArchiveLimits::default()).unwrap();
    assert_eq!(
        restored.read("image.bin").unwrap(),
        root.read("image.bin").unwrap()
    );
}

#[test]
fn unsafe_or_invalid_archives_never_write_outside_the_provisional_root() {
    for bytes in [TRAVERSAL, ABSOLUTE, &b"not a zip"[..]] {
        let folder = tempfile::tempdir().unwrap();
        let root = VaultRoot::open(folder.path()).unwrap();
        assert!(extract_zip(Cursor::new(bytes), &root, ArchiveLimits::default()).is_err());
        assert!(folder.path().read_dir().unwrap().next().is_none());
    }
}

#[test]
fn windows_separators_and_empty_archives_retain_browser_semantics() {
    let folder = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(folder.path()).unwrap();
    extract_zip(Cursor::new(WINDOWS), &root, ArchiveLimits::default()).unwrap();
    assert_eq!(
        root.read_text("Notes/today.md").unwrap(),
        "normalized Windows entry"
    );
    let empty = tempfile::tempdir().unwrap();
    let empty_root = VaultRoot::open(empty.path()).unwrap();
    extract_zip(
        Cursor::new(include_bytes!("fixtures/archives/empty.zip")),
        &empty_root,
        ArchiveLimits::default(),
    )
    .unwrap();
    let mut snapshot = create_zip(&empty_root, ArchiveLimits::default()).unwrap();
    assert_eq!(
        snapshot.file.seek(SeekFrom::End(0)).unwrap(),
        snapshot.length
    );
    assert_eq!(snapshot.length, 22);
}

#[test]
fn limits_are_enforced_before_extraction_and_on_export_without_truncation() {
    for limits in [
        ArchiveLimits {
            archive_bytes: 10,
            ..ArchiveLimits::default()
        },
        ArchiveLimits {
            expanded_bytes: 2,
            ..ArchiveLimits::default()
        },
        ArchiveLimits {
            entries: 1,
            ..ArchiveLimits::default()
        },
    ] {
        let folder = tempfile::tempdir().unwrap();
        let root = VaultRoot::open(folder.path()).unwrap();
        assert!(extract_zip(Cursor::new(NOTES), &root, limits).is_err());
        assert!(folder.path().read_dir().unwrap().next().is_none());
    }
    let folder = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(folder.path()).unwrap();
    extract_zip(Cursor::new(NOTES), &root, ArchiveLimits::default()).unwrap();
    assert!(
        create_zip(
            &root,
            ArchiveLimits {
                archive_bytes: 30,
                ..ArchiveLimits::default()
            }
        )
        .is_err()
    );
    assert_eq!(
        root.read_text("notes/café.md").unwrap(),
        "# Imported café\n"
    );
}

fn fixture(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(
                *name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn normalized_collisions_and_file_directory_conflicts_are_rejected_before_writing() {
    for bytes in [
        fixture(&[("a\\b.md", b"one"), ("a/b.md", b"two")]),
        fixture(&[("a/b.md", b"one"), ("a", b"two")]),
        fixture(&[("a", b"one"), ("a/b.md", b"two")]),
        fixture(&[("a", b"one"), ("a-", b"sibling"), ("a/b.md", b"two")]),
        fixture(&[("C:\\Users\\note.md", b"drive")]),
    ] {
        let folder = tempfile::tempdir().unwrap();
        let root = VaultRoot::open(folder.path()).unwrap();
        assert!(extract_zip(Cursor::new(bytes), &root, ArchiveLimits::default()).is_err());
        assert!(folder.path().read_dir().unwrap().next().is_none());
    }
}

#[test]
fn existing_content_is_never_overwritten_and_export_rejects_links_and_special_files() {
    let folder = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(folder.path()).unwrap();
    std::fs::write(folder.path().join("note.md"), "existing").unwrap();
    assert!(extract_zip(Cursor::new(NOTES), &root, ArchiveLimits::default()).is_err());
    assert_eq!(root.read_text("note.md").unwrap(), "existing");
    std::os::unix::fs::symlink("note.md", folder.path().join("linked.md")).unwrap();
    assert!(create_zip(&root, ArchiveLimits::default()).is_err());
    std::fs::remove_file(folder.path().join("linked.md")).unwrap();
    let _socket = std::os::unix::net::UnixListener::bind(folder.path().join("socket")).unwrap();
    assert!(create_zip(&root, ArchiveLimits::default()).is_err());
}

#[test]
fn crc_failures_reject_the_provisional_archive_payload() {
    let mut bytes = fixture(&[("note.md", b"unique payload")]);
    let offset = bytes
        .windows(14)
        .position(|slice| slice == b"unique payload")
        .unwrap();
    bytes[offset] ^= 1;
    let folder = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(folder.path()).unwrap();
    assert!(extract_zip(Cursor::new(bytes), &root, ArchiveLimits::default()).is_err());
}

#[test]
fn bulk_archive_intake_preserves_files_larger_than_the_ordinary_write_limit() {
    let source = tempfile::tempdir().unwrap();
    let file = std::fs::File::create(source.path().join("large.bin")).unwrap();
    let length = lapis_app_core::vault::MAX_FILE_BYTES + 1;
    file.set_len(length).unwrap();
    let root = VaultRoot::open(source.path()).unwrap();
    assert!(root.read("large.bin").is_err());
    let snapshot = create_zip(&root, ArchiveLimits::default()).unwrap();
    let destination = tempfile::tempdir().unwrap();
    let restored = VaultRoot::open(destination.path()).unwrap();
    extract_zip(snapshot.file, &restored, ArchiveLimits::default()).unwrap();
    assert_eq!(restored.stat("large.bin").unwrap().unwrap().size, length);
}

#[test]
fn completed_export_snapshot_does_not_hold_a_vault_open() {
    let folder = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(folder.path()).unwrap();
    extract_zip(Cursor::new(NOTES), &root, ArchiveLimits::default()).unwrap();
    let runtime = lapis_app_core::services::Runtime::default();
    let id = uuid::Uuid::new_v4();
    let vault = runtime.admit_root(id, root).unwrap();
    let work = vault.begin_work().unwrap();
    let mut snapshot = work.archive_snapshot(ArchiveLimits::default()).unwrap();
    drop(work);
    assert_eq!(vault.status().unwrap().work, 0);
    runtime.close_vault(id).unwrap();
    let mut bytes = Vec::new();
    snapshot.file.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes.len() as u64, snapshot.length);
}
