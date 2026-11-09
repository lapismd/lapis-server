use lapis_app_core::services::{
    Runtime,
    content::{ContentGrants, ContentVault},
};
use lapis_app_protocol::content::*;
use std::time::Duration;
use uuid::Uuid;

#[test]
fn disabled_startup_and_reenable_never_restore_credentials() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, directory.path()).unwrap();
    let grants = ContentGrants::disabled();
    let attach = || {
        vec![ContentVault {
            name: "Personal".into(),
            client: vault.attach().unwrap(),
        }]
    };
    assert!(!grants.status().unwrap().enabled);
    assert_eq!(
        grants
            .issue_named_read_only("Agent", attach(), Duration::from_secs(60))
            .unwrap_err()
            .code,
        ContentErrorCode::Unavailable
    );
    assert_eq!(vault.status().unwrap().clients, 0);
    grants.set_enabled(true).unwrap();
    let issued = grants
        .issue_named_read_only("Agent", attach(), Duration::from_secs(60))
        .unwrap();
    let token = issued.token.expose_secret();
    grants.check_credential(token).unwrap();
    let status = grants.status().unwrap();
    assert!(status.enabled);
    assert_eq!(status.grants.len(), 1);
    let summary = &status.grants[0];
    assert_eq!(summary.grant_id, issued.grant_id);
    assert_eq!(summary.label, "Agent");
    assert_eq!(summary.vault_ids, [id]);
    assert_eq!(summary.scope, ContentScope::Read);
    assert!(summary.active);
    assert!((1..=60).contains(&summary.expires_in_seconds));
    let json = serde_json::to_string(&status).unwrap();
    assert!(!json.contains(token));
    assert!(!json.contains("verifier"));
    grants.set_enabled(false).unwrap();
    assert!(!grants.status().unwrap().enabled);
    assert!(grants.status().unwrap().grants.is_empty());
    assert_eq!(vault.status().unwrap().clients, 0);
    assert!(grants.check_credential(token).is_err());
    grants.set_enabled(true).unwrap();
    assert!(grants.check_credential(token).is_err());
    let fresh = grants
        .issue_named_read_only("New", attach(), Duration::from_secs(60))
        .unwrap();
    grants
        .check_credential(fresh.token.expose_secret())
        .unwrap();
    grants.shutdown().unwrap();
    assert!(!grants.status().unwrap().enabled);
    assert!(grants.set_enabled(true).is_err());
    grants.set_enabled(false).unwrap();
    assert_eq!(vault.status().unwrap().clients, 0);
}

#[test]
fn named_credentials_validate_display_data_and_keep_native_scope() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), directory.path()).unwrap();
    let grants = ContentGrants::default();
    let attach = || {
        vec![ContentVault {
            name: "Personal".into(),
            client: vault.attach().unwrap(),
        }]
    };
    for label in [
        "".into(),
        "  ".into(),
        "Agent\n".into(),
        "a\0b".into(),
        "x".repeat(129),
        "🪨".repeat(33),
    ] {
        assert_eq!(
            grants
                .issue_named_read_only(&label, attach(), Duration::from_secs(60))
                .unwrap_err()
                .code,
            ContentErrorCode::InvalidRequest
        );
        assert_eq!(vault.status().unwrap().clients, 0);
    }
    assert!(
        grants
            .issue_named_read_only("Agent", vec![], Duration::from_secs(60))
            .is_err()
    );
    for lifetime in [Duration::ZERO, Duration::from_secs(86_401)] {
        assert!(
            grants
                .issue_named_read_only("Agent", attach(), lifetime)
                .is_err()
        );
    }
    let mut duplicate = attach();
    duplicate.extend(attach());
    assert!(
        grants
            .issue_named_read_only("Agent", duplicate, Duration::from_secs(60))
            .is_err()
    );
    assert_eq!(vault.status().unwrap().clients, 0);
    let issued = grants
        .issue_named_read_only(
            &format!("  {}  ", "🪨".repeat(32)),
            attach(),
            Duration::from_secs(86_400),
        )
        .unwrap();
    let summary = grants.status().unwrap().grants.remove(0);
    assert_eq!(summary.label, "🪨".repeat(32));
    assert!((1..=86_400).contains(&summary.expires_in_seconds));
    let request = ContentRequest {
        api_version: CONTENT_API_MAJOR,
        request_id: Uuid::new_v4(),
        command: ContentCommand::CreateNote {
            vault_id: vault.id(),
            path: "no.md".into(),
            text: "no".into(),
        },
    };
    let ContentOutcome::Error { error } = grants
        .execute(issued.token.expose_secret(), request)
        .outcome
    else {
        panic!("must refuse writes")
    };
    assert_eq!(error.code, ContentErrorCode::PermissionDenied);
}

#[test]
fn retiring_a_vault_revokes_whole_grants_and_preserves_unrelated_access() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let aid = Uuid::new_v4();
    let bid = Uuid::new_v4();
    let av = runtime.admit(aid, a.path()).unwrap();
    let bv = runtime.admit(bid, b.path()).unwrap();
    let grants = ContentGrants::default();
    let shared = grants
        .issue_named_read_only(
            "Shared",
            vec![
                ContentVault {
                    name: "A".into(),
                    client: av.attach().unwrap(),
                },
                ContentVault {
                    name: "B".into(),
                    client: bv.attach().unwrap(),
                },
            ],
            Duration::from_secs(60),
        )
        .unwrap();
    let independent = grants
        .issue_named_read_only(
            "Independent",
            vec![ContentVault {
                name: "B".into(),
                client: bv.attach().unwrap(),
            }],
            Duration::from_secs(60),
        )
        .unwrap();
    assert_eq!(grants.revoke_vault(aid).unwrap(), 1);
    assert!(
        grants
            .check_credential(shared.token.expose_secret())
            .is_err()
    );
    grants
        .check_credential(independent.token.expose_secret())
        .unwrap();
    assert_eq!(av.status().unwrap().clients, 0);
    assert_eq!(bv.status().unwrap().clients, 1);
    let status = grants.status().unwrap();
    assert!(status.enabled);
    assert_eq!(status.grants.len(), 1);
    assert_eq!(status.grants[0].grant_id, independent.grant_id);
    assert_eq!(grants.revoke_vault(aid).unwrap(), 0);
    assert!(grants.revoke(independent.grant_id).unwrap());
    assert!(grants.status().unwrap().grants.is_empty());
}
