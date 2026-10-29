use lapis_app_core::vault::{TextWriteConflict, VaultRoot, WriteOptions};
use std::{fs, io};

#[test]
fn native_core_reads_and_conditionally_publishes_without_a_host() {
    let directory = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(directory.path()).unwrap();
    root.mkdir("笔记", false).unwrap();
    root.write_if_unchanged(
        "笔记/café.md",
        "# Original 🪨\n".as_bytes(),
        None,
        &WriteOptions::default(),
    )
    .unwrap();
    let snapshot = root.read("笔记/café.md").unwrap();
    root.write_if_unchanged(
        "笔记/café.md",
        b"# Saved\n".as_slice(),
        Some(&snapshot),
        &WriteOptions::default(),
    )
    .unwrap();
    let failure = root
        .write_if_unchanged(
            "笔记/café.md",
            b"stale".as_slice(),
            Some(&snapshot),
            &WriteOptions::default(),
        )
        .unwrap_err();
    assert!(failure.get_ref().unwrap().is::<TextWriteConflict>());
    assert_eq!(root.read_text("笔记/café.md").unwrap(), "# Saved\n");
    assert_eq!(root.list("笔记").unwrap().files, ["café.md"]);
}

#[test]
fn failed_staging_preserves_the_saved_file_and_cleans_temporary_entries() {
    struct Broken;
    impl io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("fixture read failure"))
        }
    }
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("note.md"), "saved").unwrap();
    let root = VaultRoot::open(directory.path()).unwrap();
    assert!(
        root.write_from("note.md", Broken, &WriteOptions::default())
            .is_err()
    );
    assert_eq!(root.read_text("note.md").unwrap(), "saved");
    assert_eq!(root.list("").unwrap().files, ["note.md"]);
}

#[test]
fn exclusive_publication_does_not_replace_a_destination() {
    let directory = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(directory.path()).unwrap();
    root.create_file("one.md", b"one".as_slice(), &WriteOptions::default())
        .unwrap();
    root.create_file("two.md", b"two".as_slice(), &WriteOptions::default())
        .unwrap();
    assert!(root.rename("one.md", "two.md").is_err());
    assert!(root.copy_file("one.md", "two.md").is_err());
    assert_eq!(root.read_text("one.md").unwrap(), "one");
    assert_eq!(root.read_text("two.md").unwrap(), "two");
}

#[cfg(unix)]
#[test]
fn traversal_and_symlinks_do_not_grant_file_or_native_directory_access() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), "outside").unwrap();
    std::os::unix::fs::symlink(outside.path(), directory.path().join("escape")).unwrap();
    let root = VaultRoot::open(directory.path()).unwrap();
    for path in ["../secret", "/secret", "escape/secret"] {
        assert!(root.read(path).is_err());
        assert!(root.write(path, b"bad", &WriteOptions::default()).is_err());
        assert!(root.open_directory(path).is_err());
        assert!(root.entry_metadata(path).is_err());
    }
    assert_eq!(
        fs::read_to_string(outside.path().join("secret")).unwrap(),
        "outside"
    );
}

#[test]
fn native_observers_preserve_the_original_physical_root() {
    let directory = tempfile::tempdir().unwrap();
    let selected = directory.path().join("selected");
    fs::create_dir(&selected).unwrap();
    let root = VaultRoot::open(&selected).unwrap();
    let observer = root.try_clone().unwrap();
    assert_eq!(
        root.physical_identity().unwrap(),
        observer.physical_identity().unwrap()
    );
    fs::rename(&selected, directory.path().join("original")).unwrap();
    fs::create_dir(&selected).unwrap();
    assert!(root.verify_physical_root().is_err());
    assert!(observer.verify_physical_root().is_err());
}
