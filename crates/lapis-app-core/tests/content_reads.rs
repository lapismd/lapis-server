use lapis_app_core::services::{
    Runtime,
    content::{ContentSession, ContentVault},
};
use lapis_app_protocol::content::*;
use std::{fs, time::Duration};
use uuid::Uuid;

fn request(command: ContentCommand) -> ContentRequest {
    ContentRequest {
        api_version: CONTENT_API_MAJOR,
        request_id: Uuid::new_v4(),
        command,
    }
}
fn page(cursor: Option<String>) -> ContentPageQuery {
    ContentPageQuery { limit: 1, cursor }
}
fn value(reply: ContentReply) -> ContentValue {
    match reply.outcome {
        ContentOutcome::Ok { value } => value,
        ContentOutcome::Error { error } => panic!("{error:?}"),
    }
}
fn error(reply: ContentReply) -> ContentErrorCode {
    match reply.outcome {
        ContentOutcome::Error { error } => {
            assert_eq!(error.effect, ContentEffectStatus::NotStarted);
            error.code
        }
        _ => panic!("expected refusal"),
    }
}

#[test]
fn session_reads_authoritative_content_and_only_advertises_read_operations() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("note.md"), "# saved 🪨").unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, directory.path()).unwrap();
    let expected = vault.attach().unwrap().read_text("note.md").unwrap();
    let session = ContentSession::read_only(
        vec![ContentVault {
            name: "Personal".into(),
            client: vault.attach().unwrap(),
        }],
        Duration::from_secs(60),
    )
    .unwrap();
    let input = request(ContentCommand::ReadNote {
        vault_id: id,
        path: "note.md".into(),
    });
    let correlation = input.request_id;
    let reply = session.execute(input);
    assert_eq!(reply.request_id, correlation);
    let ContentValue::Note { text, version, .. } = value(reply) else {
        panic!()
    };
    assert_eq!(text, expected.text);
    assert_eq!(version, expected.version);
    let ContentValue::Capabilities { capabilities } =
        value(session.execute(request(ContentCommand::Capabilities {})))
    else {
        panic!()
    };
    assert_eq!(capabilities.operations.len(), 4);
    assert!(
        capabilities
            .operations
            .iter()
            .all(|d| d.effect == ContentEffect::ReadOnly && d.scope == ContentScope::Read)
    );
    assert_eq!(
        error(session.execute(request(ContentCommand::CreateNote {
            vault_id: id,
            path: "new.md".into(),
            text: "no".into()
        }))),
        ContentErrorCode::PermissionDenied
    );
    assert!(!directory.path().join("new.md").exists());
}

#[test]
fn grants_exclude_other_vaults_and_retire_on_revoke_expiry_and_vault_close() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("note.md"), "saved").unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, directory.path()).unwrap();
    let grant = || {
        vec![ContentVault {
            name: "Personal".into(),
            client: vault.attach().unwrap(),
        }]
    };
    let session = ContentSession::read_only(grant(), Duration::from_secs(60)).unwrap();
    assert_eq!(
        error(session.execute(request(ContentCommand::ReadNote {
            vault_id: Uuid::new_v4(),
            path: "note.md".into()
        }))),
        ContentErrorCode::PermissionDenied
    );
    session.revoke().unwrap();
    assert_eq!(vault.status().unwrap().clients, 0);
    assert_eq!(
        error(session.execute(request(ContentCommand::Capabilities {}))),
        ContentErrorCode::Unauthenticated
    );
    let expired = ContentSession::read_only(grant(), Duration::from_millis(1)).unwrap();
    std::thread::sleep(Duration::from_millis(10));
    assert_eq!(
        error(expired.execute(request(ContentCommand::Capabilities {}))),
        ContentErrorCode::Unauthenticated
    );
    drop(expired);
    let session = ContentSession::read_only(grant(), Duration::from_secs(60)).unwrap();
    runtime.close_vault(id).unwrap();
    assert_eq!(
        error(session.execute(request(ContentCommand::ReadNote {
            vault_id: id,
            path: "note.md".into()
        }))),
        ContentErrorCode::Unavailable
    );
}

#[test]
fn listings_are_bounded_sorted_and_cursors_are_bound_to_the_session_and_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir(directory.path().join("Folder")).unwrap();
    for name in ["b.md", "a.md", "Folder/c.md", "image.png"] {
        fs::write(directory.path().join(name), "text").unwrap();
    }
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, directory.path()).unwrap();
    let grant = || {
        ContentSession::read_only(
            vec![ContentVault {
                name: "Personal".into(),
                client: vault.attach().unwrap(),
            }],
            Duration::from_secs(60),
        )
        .unwrap()
    };
    let session = grant();
    let list = |cursor| {
        request(ContentCommand::ListNotes {
            vault_id: id,
            directory: None,
            page: page(cursor),
        })
    };
    let ContentValue::Notes {
        items,
        next_cursor: Some(cursor),
    } = value(session.execute(list(None)))
    else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].path, "a.md");
    assert_eq!(
        error(grant().execute(list(Some(cursor.clone())))),
        ContentErrorCode::Conflict
    );
    let wrong_directory = request(ContentCommand::ListNotes {
        vault_id: id,
        directory: Some("Folder".into()),
        page: page(Some(cursor.clone())),
    });
    assert_eq!(
        error(session.execute(wrong_directory)),
        ContentErrorCode::Conflict
    );
    let ContentValue::Notes { items, next_cursor } =
        value(session.execute(list(Some(cursor.clone()))))
    else {
        panic!()
    };
    assert_eq!(items[0].path, "b.md");
    assert!(next_cursor.is_none());
    fs::write(directory.path().join("aa.md"), "new").unwrap();
    assert_eq!(
        error(session.execute(list(Some(cursor)))),
        ContentErrorCode::Conflict
    );
}

#[test]
fn private_paths_symlinks_non_utf8_and_oversized_content_are_refused() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.md"), "private").unwrap();
    std::os::unix::fs::symlink(outside.path(), directory.path().join("escape")).unwrap();
    fs::write(directory.path().join("binary.md"), [0xff]).unwrap();
    fs::write(
        directory.path().join("large.md"),
        vec![b'a'; MAX_CONTENT_TEXT_BYTES + 1],
    )
    .unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, directory.path()).unwrap();
    let session = ContentSession::read_only(
        vec![ContentVault {
            name: "Personal".into(),
            client: vault.attach().unwrap(),
        }],
        Duration::from_secs(60),
    )
    .unwrap();
    for (path, code) in [
        (".lapis/private", ContentErrorCode::InvalidRequest),
        ("escape/secret.md", ContentErrorCode::PermissionDenied),
        ("binary.md", ContentErrorCode::InvalidRequest),
        ("large.md", ContentErrorCode::TooLarge),
    ] {
        assert_eq!(
            error(session.execute(request(ContentCommand::ReadNote {
                vault_id: id,
                path: path.into()
            }))),
            code
        );
    }
    fs::rename(directory.path(), outside.path().join("moved")).unwrap();
    fs::create_dir(directory.path()).unwrap();
    assert_eq!(
        error(session.execute(request(ContentCommand::ReadNote {
            vault_id: id,
            path: "binary.md".into()
        }))),
        ContentErrorCode::PermissionDenied
    );
}

#[test]
fn host_grants_are_bounded_and_vault_pages_do_not_expose_unadmitted_roots() {
    let runtime = Runtime::default();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let a = runtime.admit(Uuid::new_v4(), first.path()).unwrap();
    let b = runtime.admit(Uuid::new_v4(), second.path()).unwrap();
    let grant = || {
        vec![
            ContentVault {
                name: "A".into(),
                client: a.attach().unwrap(),
            },
            ContentVault {
                name: "B".into(),
                client: b.attach().unwrap(),
            },
        ]
    };
    assert!(ContentSession::read_only(grant(), Duration::ZERO).is_err());
    assert!(ContentSession::read_only(grant(), Duration::from_secs(86_401)).is_err());
    assert!(
        ContentSession::read_only(
            vec![
                ContentVault {
                    name: "A".into(),
                    client: a.attach().unwrap()
                },
                ContentVault {
                    name: "duplicate".into(),
                    client: a.attach().unwrap()
                }
            ],
            Duration::from_secs(60)
        )
        .is_err()
    );
    let session = ContentSession::read_only(grant(), Duration::from_secs(60)).unwrap();
    let ContentValue::Vaults {
        items,
        next_cursor: Some(cursor),
    } = value(session.execute(request(ContentCommand::ListVaults { page: page(None) })))
    else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    let first_id = items[0].vault_id;
    let ContentValue::Vaults {
        items,
        next_cursor: None,
    } = value(session.execute(request(ContentCommand::ListVaults {
        page: page(Some(cursor)),
    })))
    else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    assert_ne!(items[0].vault_id, first_id);
    drop(session);
    assert_eq!(a.status().unwrap().clients, 0);
    assert_eq!(b.status().unwrap().clients, 0);
}
