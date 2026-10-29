use lapis_app_core::vault::{VaultRoot, WriteOptions};
use std::fs;

#[test]
fn mutations_synchronize_readable_parent_handles() {
    let temp = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(temp.path()).unwrap();
    let options = WriteOptions::default();
    root.mkdir("from", false).unwrap();
    root.mkdir("to", false).unwrap();
    root.create_file("from/Note.md", &b"first"[..], &options)
        .unwrap();
    root.write_if_unchanged("from/Note.md", &b"second"[..], Some(b"first"), &options)
        .unwrap();
    root.append("from/Note.md", " appended", &options).unwrap();
    root.copy_file("from/Note.md", "Copy.md").unwrap();
    root.rename("from/Note.md", "to/Note.md").unwrap();
    assert_eq!(
        fs::read(temp.path().join("to/Note.md")).unwrap(),
        b"second appended"
    );
    assert_eq!(
        fs::read(temp.path().join("Copy.md")).unwrap(),
        b"second appended"
    );
    root.remove_file("Copy.md").unwrap();
    root.remove_directory("from", false).unwrap();
    root.remove_directory("to", true).unwrap();
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}
