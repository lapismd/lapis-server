use lapis_app_core::services::{
    Runtime,
    content::{ContentGrants, ContentVault, MAX_CONTENT_GRANTS},
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
fn capabilities() -> ContentRequest {
    request(ContentCommand::Capabilities {})
}
fn error(reply: ContentReply) -> ContentErrorCode {
    let ContentOutcome::Error { error } = reply.outcome else {
        panic!("expected refusal")
    };
    assert_eq!(error.effect, ContentEffectStatus::NotStarted);
    error.code
}
fn value(reply: ContentReply) -> ContentValue {
    match reply.outcome {
        ContentOutcome::Ok { value } => value,
        ContentOutcome::Error { error } => panic!("{error:?}"),
    }
}

#[test]
fn credentials_reuse_scoped_reads_and_cannot_authorize_other_vaults_or_writes() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("note.md"), "saved 🪨").unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, directory.path()).unwrap();
    let grants = ContentGrants::default();
    let issued = grants
        .issue_read_only(
            vec![ContentVault {
                name: "Personal".into(),
                client: vault.attach().unwrap(),
            }],
            Duration::from_secs(60),
        )
        .unwrap();
    let token = issued.token.expose_secret();
    assert!(!format!("{issued:?}").contains(token));
    let input = request(ContentCommand::ReadNote {
        vault_id: id,
        path: "note.md".into(),
    });
    let correlation = input.request_id;
    let reply = grants.execute(token, input);
    assert_eq!(reply.request_id, correlation);
    let ContentValue::Note { text, version, .. } = value(reply) else {
        panic!()
    };
    assert_eq!(text, "saved 🪨");
    assert_eq!(
        version,
        vault
            .attach()
            .unwrap()
            .read_text("note.md")
            .unwrap()
            .version
    );
    let ContentValue::Capabilities {
        capabilities: advertised,
    } = value(grants.execute(token, capabilities()))
    else {
        panic!()
    };
    assert_eq!(advertised.operations.len(), 4);
    assert!(
        advertised
            .operations
            .iter()
            .all(|op| op.scope == ContentScope::Read)
    );
    assert_eq!(
        error(grants.execute(
            token,
            request(ContentCommand::ReadNote {
                vault_id: Uuid::new_v4(),
                path: "note.md".into()
            })
        )),
        ContentErrorCode::PermissionDenied
    );
    assert_eq!(
        error(grants.execute(
            token,
            request(ContentCommand::CreateNote {
                vault_id: id,
                path: "new.md".into(),
                text: "no".into()
            })
        )),
        ContentErrorCode::PermissionDenied
    );
    assert!(!directory.path().join("new.md").exists());
    assert!(grants.revoke(issued.grant_id).unwrap());
    assert!(!grants.revoke(issued.grant_id).unwrap());
    assert_eq!(vault.status().unwrap().clients, 0);
    assert_eq!(
        error(grants.execute(token, capabilities())),
        ContentErrorCode::Unauthenticated
    );
}

#[test]
fn credentials_are_instance_bound_and_reject_invalid_input_before_dispatch() {
    let grants = ContentGrants::default();
    let issued = grants
        .issue_read_only(vec![], Duration::from_secs(60))
        .unwrap();
    let another = grants
        .issue_read_only(vec![], Duration::from_secs(60))
        .unwrap();
    assert_ne!(issued.token.expose_secret(), another.token.expose_secret());
    for token in [
        "",
        "owner-token",
        &"x".repeat(4096),
        &issued.grant_id.to_string(),
        &format!("{} ", issued.token.expose_secret()),
    ] {
        let mut input = capabilities();
        input.api_version = 0;
        assert_eq!(
            error(grants.execute(token, input)),
            ContentErrorCode::Unauthenticated
        );
    }
    let fresh = ContentGrants::default();
    assert_eq!(
        error(fresh.execute(issued.token.expose_secret(), capabilities())),
        ContentErrorCode::Unauthenticated
    );
    let mut invalid = capabilities();
    invalid.api_version = 0;
    assert_eq!(
        error(grants.execute(issued.token.expose_secret(), invalid)),
        ContentErrorCode::ProtocolMismatch
    );
    grants.revoke(issued.grant_id).unwrap();
    value(grants.execute(another.token.expose_secret(), capabilities()));
    grants.shutdown().unwrap();
    assert_eq!(
        error(grants.execute(another.token.expose_secret(), capabilities())),
        ContentErrorCode::Unauthenticated
    );
    assert_eq!(
        grants
            .issue_read_only(vec![], Duration::from_secs(60))
            .unwrap_err()
            .code,
        ContentErrorCode::Unavailable
    );
    grants.shutdown().unwrap();
}

#[test]
fn grant_budget_fails_closed_and_revocation_releases_capacity() {
    let grants = ContentGrants::default();
    let mut issued = Vec::new();
    for _ in 0..MAX_CONTENT_GRANTS {
        issued.push(
            grants
                .issue_read_only(vec![], Duration::from_secs(60))
                .unwrap(),
        );
    }
    assert_eq!(
        grants
            .issue_read_only(vec![], Duration::from_secs(60))
            .unwrap_err()
            .code,
        ContentErrorCode::Busy
    );
    grants.revoke(issued[0].grant_id).unwrap();
    let replacement = grants
        .issue_read_only(vec![], Duration::from_secs(60))
        .unwrap();
    value(grants.execute(replacement.token.expose_secret(), capabilities()));
    assert_eq!(
        error(grants.execute(issued[0].token.expose_secret(), capabilities())),
        ContentErrorCode::Unauthenticated
    );
}

#[test]
fn shutdown_and_drop_release_all_dedicated_attachments() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), directory.path()).unwrap();
    let attach = || {
        vec![ContentVault {
            name: "test".into(),
            client: vault.attach().unwrap(),
        }]
    };
    let grants = ContentGrants::default();
    for _ in 0..2 {
        grants
            .issue_read_only(attach(), Duration::from_secs(60))
            .unwrap();
    }
    assert_eq!(vault.status().unwrap().clients, 2);
    grants.shutdown().unwrap();
    assert_eq!(vault.status().unwrap().clients, 0);
    let grants = ContentGrants::default();
    grants
        .issue_read_only(attach(), Duration::from_secs(60))
        .unwrap();
    drop(grants);
    assert_eq!(vault.status().unwrap().clients, 0);
}

#[test]
fn credentials_for_the_same_vault_do_not_share_pagination_authority() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("a.md"), "a").unwrap();
    fs::write(directory.path().join("b.md"), "b").unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, directory.path()).unwrap();
    let grants = ContentGrants::default();
    let issue = || {
        grants
            .issue_read_only(
                vec![ContentVault {
                    name: "Personal".into(),
                    client: vault.attach().unwrap(),
                }],
                Duration::from_secs(60),
            )
            .unwrap()
    };
    let a = issue();
    let b = issue();
    let list = |cursor| {
        request(ContentCommand::ListNotes {
            vault_id: id,
            directory: None,
            page: ContentPageQuery { limit: 1, cursor },
        })
    };
    let ContentValue::Notes {
        next_cursor: Some(cursor),
        ..
    } = value(grants.execute(a.token.expose_secret(), list(None)))
    else {
        panic!()
    };
    assert_eq!(
        error(grants.execute(b.token.expose_secret(), list(Some(cursor.clone())))),
        ContentErrorCode::Conflict
    );
    let ContentValue::Notes {
        items,
        next_cursor: None,
    } = value(grants.execute(a.token.expose_secret(), list(Some(cursor))))
    else {
        panic!()
    };
    assert_eq!(items[0].path, "b.md");
    runtime.close_vault(id).unwrap();
    assert_eq!(
        error(grants.execute(
            b.token.expose_secret(),
            request(ContentCommand::ReadNote {
                vault_id: id,
                path: "a.md".into()
            })
        )),
        ContentErrorCode::Unavailable
    );
}
