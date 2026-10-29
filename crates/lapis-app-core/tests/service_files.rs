use lapis_app_core::services::Runtime;
use lapis_app_protocol::files::{FileCommand, FileOptions, FileValue};
use lapis_app_protocol::{ErrorCode, Outcome, PROTOCOL_MAJOR, Request, VaultCommand, VaultValue};
use uuid::Uuid;

fn options() -> FileOptions {
    FileOptions::default()
}
fn invoke(client: &lapis_app_core::services::Client, operation: FileCommand) -> FileValue {
    let descriptor = client.descriptor();
    let request = Request {
        protocol_version: PROTOCOL_MAJOR,
        request_id: Uuid::new_v4(),
        vault_id: descriptor.vault_id,
        generation: descriptor.generation,
        command: VaultCommand::Files { operation },
    };
    match client.execute(request).outcome {
        Outcome::Ok {
            value: VaultValue::Files { value },
        } => value,
        other => panic!("unexpected file result: {other:?}"),
    }
}
#[test]
fn portable_file_actions_keep_the_existing_shell_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), dir.path()).unwrap();
    let client = vault.attach().unwrap();
    invoke(
        &client,
        FileCommand::Mkdir {
            path: "Notes".into(),
            recursive: false,
        },
    );
    invoke(
        &client,
        FileCommand::CreateFile {
            path: "Notes/é.md".into(),
            data: "# 💎".as_bytes().to_vec(),
            options: options(),
        },
    );
    invoke(
        &client,
        FileCommand::AppendText {
            path: "Notes/é.md".into(),
            data: "\nnext".into(),
            options: options(),
        },
    );
    let FileValue::Text { data } = invoke(
        &client,
        FileCommand::ReadText {
            path: "Notes/é.md".into(),
        },
    ) else {
        panic!()
    };
    assert_eq!(data, "# 💎\nnext");
    invoke(
        &client,
        FileCommand::WriteTextIfUnchanged {
            path: "Notes/é.md".into(),
            data: "saved".into(),
            expected: Some(data),
            options: options(),
        },
    );
    invoke(
        &client,
        FileCommand::Copy {
            path: "Notes/é.md".into(),
            new_path: "copy.md".into(),
        },
    );
    invoke(
        &client,
        FileCommand::Rename {
            path: "copy.md".into(),
            new_path: "moved.md".into(),
        },
    );
    let FileValue::List { listing } = invoke(&client, FileCommand::List { path: "".into() }) else {
        panic!()
    };
    assert_eq!(listing.files, vec!["moved.md"]);
    assert_eq!(listing.folders, vec!["Notes"]);
    let FileValue::Stat { stat: Some(stat) } = invoke(
        &client,
        FileCommand::Stat {
            path: "moved.md".into(),
        },
    ) else {
        panic!()
    };
    assert_eq!(stat.size, 5);
    assert_eq!(stat.kind, "file");
    let FileValue::Binary { data } = invoke(
        &client,
        FileCommand::ReadRange {
            path: "moved.md".into(),
            offset: 1,
            length: 3,
        },
    ) else {
        panic!()
    };
    assert_eq!(data, b"ave");
    invoke(
        &client,
        FileCommand::WriteBinary {
            path: "data.bin".into(),
            data: vec![0, 255, 4],
            options: options(),
        },
    );
    let FileValue::Binary { data } = invoke(
        &client,
        FileCommand::ReadBinary {
            path: "data.bin".into(),
        },
    ) else {
        panic!()
    };
    assert_eq!(data, vec![0, 255, 4]);
    invoke(
        &client,
        FileCommand::Remove {
            path: "moved.md".into(),
        },
    );
    invoke(
        &client,
        FileCommand::Rmdir {
            path: "Notes".into(),
            recursive: true,
        },
    );
    assert!(matches!(
        invoke(
            &client,
            FileCommand::Exists {
                path: "Notes".into()
            }
        ),
        FileValue::Exists { exists: false }
    ));
}
#[test]
fn file_mutations_preserve_dirty_paths_descendants_collisions_and_external_bytes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Notes")).unwrap();
    std::fs::write(dir.path().join("Notes/note.md"), "saved").unwrap();
    std::fs::write(dir.path().join("occupied.md"), "external").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), dir.path()).unwrap();
    let client = vault.attach().unwrap();
    let editor = client.open_editor("Notes/note.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    let other = vault.attach().unwrap();
    for operation in [
        FileCommand::Rename {
            path: "Notes".into(),
            new_path: "New".into(),
        },
        FileCommand::Rmdir {
            path: "Notes".into(),
            recursive: true,
        },
        FileCommand::WriteText {
            path: "Notes/note.md".into(),
            data: "unsafe".into(),
            options: options(),
        },
    ] {
        assert_eq!(other.files(operation).unwrap_err().code(), ErrorCode::Busy);
    }
    client.discard_editor(editor).unwrap();
    assert_eq!(
        other
            .files(FileCommand::Copy {
                path: "Notes/note.md".into(),
                new_path: "occupied.md".into()
            })
            .unwrap_err()
            .code(),
        ErrorCode::AlreadyExists
    );
    assert_eq!(
        other
            .files(FileCommand::WriteTextIfUnchanged {
                path: "Notes/note.md".into(),
                data: "stale".into(),
                expected: Some("old".into()),
                options: options()
            })
            .unwrap_err()
            .code(),
        ErrorCode::Conflict
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("occupied.md")).unwrap(),
        "external"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("Notes/note.md")).unwrap(),
        "saved"
    );
}

#[test]
fn portable_commands_preserve_options_limits_and_physical_admission() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), dir.path()).unwrap();
    let client = vault.attach().unwrap();
    invoke(
        &client,
        FileCommand::WriteText {
            path: "note.md".into(),
            data: "saved".into(),
            options: FileOptions {
                mtime: Some(1_700_000_000_000.0),
            },
        },
    );
    let FileValue::Stat { stat: Some(stat) } = invoke(
        &client,
        FileCommand::Stat {
            path: "note.md".into(),
        },
    ) else {
        panic!()
    };
    assert_eq!(stat.mtime, 1_700_000_000_000.0);
    assert_eq!(
        client
            .files(FileCommand::ReadRange {
                path: "note.md".into(),
                offset: 0,
                length: 256 * 1024 + 1
            })
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        client
            .files(FileCommand::ReadText {
                path: "../outside.md".into()
            })
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    std::os::unix::fs::symlink("note.md", dir.path().join("alias.md")).unwrap();
    assert_eq!(
        client
            .files(FileCommand::ReadText {
                path: "alias.md".into()
            })
            .unwrap_err()
            .code(),
        ErrorCode::PermissionDenied
    );
    let moved = dir.path().with_extension("moved");
    std::fs::rename(dir.path(), &moved).unwrap();
    std::fs::create_dir(dir.path()).unwrap();
    assert_eq!(
        client
            .files(FileCommand::WriteText {
                path: "note.md".into(),
                data: "unsafe".into(),
                options: options()
            })
            .unwrap_err()
            .code(),
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        std::fs::read_to_string(moved.join("note.md")).unwrap(),
        "saved"
    );
    std::fs::remove_dir_all(moved).unwrap();
}
