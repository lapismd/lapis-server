use lapis_app_core::{repository::remotes::selection::Selection, vault::VaultRoot};
use lapis_app_protocol::repository_remotes::{
    RemoteAuthentication, RemoteConnection, RemoteRepositoryTarget as Target,
};
use std::{fs, os::unix::fs::PermissionsExt as _, path::PathBuf};
use uuid::Uuid;

fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf, Selection) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let vault = root.join("notes");
    let private = root.join("private");
    fs::create_dir(&vault).unwrap();
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
    let selection = Selection::open(
        VaultRoot::open(&private).unwrap(),
        &VaultRoot::open(&vault).unwrap(),
    )
    .unwrap();
    (temporary, vault, private, selection)
}
fn connection(original: bool) -> RemoteConnection {
    RemoteConnection {
        name: if original { "origin" } else { "backup" }.into(),
        url: "https://unreachable.invalid/notes.git".into(),
        branch: "main".into(),
        bookmark: if original { "main" } else { "lapis-backup" }.into(),
        authentication: RemoteAuthentication::Token,
        credential: Uuid::from_u128(17).to_string(),
    }
}

#[test]
fn target_selection_stays_private_and_new_connections_require_explicit_automatic_opt_in() {
    let (_temporary, vault, private, selection) = fixture();
    assert_eq!(selection.read().unwrap().revision, 0);
    let saved = selection
        .configure(0, Target::Backup, connection(false), || Ok(false))
        .unwrap();
    assert!(!saved.automatic_sync);
    let enabled = selection.set_automatic(saved.revision, true).unwrap();
    let original = selection
        .configure(enabled.revision, Target::Original, connection(true), || {
            Ok(false)
        })
        .unwrap();
    assert!(
        original.automatic_sync,
        "original selection must not change the backup preference"
    );
    assert_eq!(original.backup.as_ref().unwrap().name, "backup");
    assert_eq!(original.original.as_ref().unwrap().name, "origin");
    let mut replacement = connection(false);
    replacement.branch = "another".into();
    let changed = selection
        .configure(original.revision, Target::Backup, replacement, || Ok(false))
        .unwrap();
    assert!(
        !changed.automatic_sync,
        "a changed destination is a new manual connection"
    );
    assert_eq!(changed.original.unwrap().name, "origin");
    assert!(
        fs::read_dir(&vault).unwrap().next().is_none(),
        "saving must not initialize or write a repository"
    );
    drop(selection);
    let reopened = Selection::open(
        VaultRoot::open(&private).unwrap(),
        &VaultRoot::open(&vault).unwrap(),
    )
    .unwrap();
    let persisted = reopened.read().unwrap();
    assert_eq!(persisted.revision, changed.revision);
    assert_eq!(persisted.backup.unwrap().branch, "another");
    let bytes = fs::read_to_string(private.join("repository-remotes.json")).unwrap();
    assert!(!bytes.contains(private.to_str().unwrap()));
    assert!(!bytes.contains(vault.to_str().unwrap()));
}

#[test]
fn stale_and_pending_publications_cannot_rebind_either_target() {
    let (_temporary, _vault, private, selection) = fixture();
    selection
        .configure(0, Target::Backup, connection(false), || Ok(false))
        .unwrap();
    let before = fs::read(private.join("repository-remotes.json")).unwrap();
    assert!(
        selection
            .configure(0, Target::Original, connection(true), || Ok(false))
            .is_err()
    );
    assert!(
        selection
            .configure(1, Target::Backup, connection(false), || Ok(true))
            .is_err()
    );
    assert_eq!(
        fs::read(private.join("repository-remotes.json")).unwrap(),
        before
    );
    let mut data: serde_json::Value = serde_json::from_slice(&before).unwrap();
    data["revision"] = serde_json::json!(9_007_199_254_740_991_u64);
    let exhausted = serde_json::to_vec(&data).unwrap();
    fs::write(private.join("repository-remotes.json"), &exhausted).unwrap();
    assert!(selection.advance(9_007_199_254_740_991).is_err());
    assert_eq!(
        fs::read(private.join("repository-remotes.json")).unwrap(),
        exhausted
    );
}

#[test]
fn credential_references_and_endpoints_fail_closed_without_storing_secret_inputs() {
    let (_temporary, _vault, private, selection) = fixture();
    let mut invalid = connection(false);
    invalid.url = "https://user:synthetic-secret@forge.example/notes.git".into();
    assert!(
        selection
            .configure(0, Target::Backup, invalid, || Ok(false))
            .is_err()
    );
    let mut invalid = connection(false);
    invalid.credential = "synthetic-token-bytes".into();
    assert!(
        selection
            .configure(0, Target::Backup, invalid, || Ok(false))
            .is_err()
    );
    let mut anonymous = connection(false);
    anonymous.authentication = RemoteAuthentication::Anonymous;
    anonymous.credential.clear();
    let saved = selection
        .configure(0, Target::Backup, anonymous, || Ok(false))
        .unwrap();
    assert!(selection.set_automatic(saved.revision, true).is_err());
    assert!(
        !fs::read_to_string(private.join("repository-remotes.json"))
            .unwrap()
            .contains("synthetic")
    );
}

#[test]
fn legacy_preferences_import_once_and_private_storage_cannot_be_inside_a_vault() {
    let (_temporary, vault, private, selection) = fixture();
    let migrated = selection
        .import_legacy(0, Some(connection(false)), Some(true))
        .unwrap();
    assert!(migrated.automatic_sync);
    let mut later = connection(false);
    later.branch = "legacy-must-not-replace".into();
    let repeated = selection
        .import_legacy(0, Some(later), Some(false))
        .unwrap();
    assert_eq!(repeated.revision, migrated.revision);
    assert_eq!(repeated.backup.unwrap().branch, "main");
    assert!(repeated.automatic_sync);
    let nested = vault.join("private");
    fs::create_dir(&nested).unwrap();
    fs::set_permissions(&nested, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
        Selection::open(
            VaultRoot::open(&nested).unwrap(),
            &VaultRoot::open(&vault).unwrap()
        )
        .is_err()
    );
    fs::set_permissions(&private, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        Selection::open(
            VaultRoot::open(&private).unwrap(),
            &VaultRoot::open(&vault).unwrap()
        )
        .is_err()
    );
}

#[test]
fn pending_checks_run_under_the_selection_lock_and_failed_checks_preserve_bytes() {
    let (_temporary, vault, private, selection) = fixture();
    selection
        .configure(0, Target::Backup, connection(false), || Ok(false))
        .unwrap();
    let before = fs::read(private.join("repository-remotes.json")).unwrap();
    let second = Selection::open(
        VaultRoot::open(&private).unwrap(),
        &VaultRoot::open(&vault).unwrap(),
    )
    .unwrap();
    let saved = selection
        .configure(1, Target::Original, connection(true), || {
            assert!(
                second.advance(1).is_err(),
                "another writer must stay fenced"
            );
            Ok(false)
        })
        .unwrap();
    assert_eq!(saved.revision, 2);
    let committed = fs::read(private.join("repository-remotes.json")).unwrap();
    assert_ne!(committed, before);
    assert!(
        selection
            .configure(2, Target::Backup, connection(false), || {
                Err(std::io::Error::other("publication state unavailable"))
            })
            .is_err()
    );
    assert_eq!(
        fs::read(private.join("repository-remotes.json")).unwrap(),
        committed
    );
    assert_eq!(second.advance(2).unwrap().revision, 3);
    assert!(selection.advance(2).is_err());
}

#[test]
fn anonymous_legacy_selection_never_enables_automatic_publication() {
    let (_temporary, _vault, _private, selection) = fixture();
    let mut anonymous = connection(false);
    anonymous.authentication = RemoteAuthentication::Anonymous;
    anonymous.credential.clear();
    let saved = selection
        .import_legacy(0, Some(anonymous), Some(true))
        .unwrap();
    assert!(!saved.automatic_sync);
    assert!(selection.set_automatic(saved.revision, true).is_err());
}

#[test]
fn host_ssh_selection_needs_no_forge_account_and_stays_manual() {
    let (_temporary, _vault, _private, selection) = fixture();
    let mut ssh = connection(false);
    ssh.url = "ssh://git@forge.example/team/notes.git".into();
    ssh.authentication = RemoteAuthentication::SshAgent;
    ssh.credential.clear();
    let saved = selection
        .configure(0, Target::Backup, ssh.clone(), || Ok(false))
        .unwrap();
    assert!(!saved.automatic_sync);
    assert_eq!(
        saved.backup.as_ref().unwrap().authentication,
        RemoteAuthentication::SshAgent
    );
    ssh.authentication = RemoteAuthentication::SshDefault;
    let saved = selection
        .configure(saved.revision, Target::Backup, ssh.clone(), || Ok(false))
        .unwrap();
    assert!(!saved.automatic_sync);
    ssh.authentication = RemoteAuthentication::Token;
    ssh.credential = Uuid::new_v4().to_string();
    assert!(
        selection
            .configure(saved.revision, Target::Backup, ssh, || Ok(false))
            .is_err()
    );
    assert_eq!(selection.read().unwrap().revision, saved.revision);
}

#[test]
fn original_discovery_and_config_save_share_one_cross_process_selection_guard() {
    use lapis_app_core::repository::remotes::selection::OriginalObservation;
    let (_temporary, vault, private, selection) = fixture();
    let observed = OriginalObservation {
        source: "a".repeat(64),
        configuration: "b".repeat(64),
    };
    assert!(
        selection
            .observe_original(Some(OriginalObservation {
                source: "invalid".into(),
                configuration: "b".repeat(64)
            }))
            .is_err()
    );
    assert_eq!(selection.read().unwrap().revision, 0);
    let discovered = selection.observe_original(Some(observed.clone())).unwrap();
    assert_eq!(
        selection
            .observe_original(Some(observed.clone()))
            .unwrap()
            .revision,
        discovered.revision
    );
    let next = OriginalObservation {
        source: observed.source.clone(),
        configuration: "c".repeat(64),
    };
    let second = Selection::open(
        VaultRoot::open(&private).unwrap(),
        &VaultRoot::open(&vault).unwrap(),
    )
    .unwrap();
    assert!(
        selection
            .configure_original(
                0,
                connection(true),
                observed.clone(),
                || panic!("stale pending callback"),
                || panic!("stale config write")
            )
            .is_err()
    );
    assert!(
        selection
            .configure_original(
                discovered.revision,
                connection(true),
                observed.clone(),
                || Ok(true),
                || panic!("pending publication must block native config writes")
            )
            .is_err()
    );
    let before = fs::read(private.join("repository-remotes.json")).unwrap();
    assert!(
        selection
            .configure_original(
                discovered.revision,
                connection(true),
                observed.clone(),
                || Ok(false),
                || Err(std::io::Error::other("synthetic native save failure"))
            )
            .is_err()
    );
    assert_eq!(
        fs::read(private.join("repository-remotes.json")).unwrap(),
        before
    );
    let saved = selection
        .configure_original(
            discovered.revision,
            connection(true),
            observed,
            || Ok(false),
            || {
                assert!(
                    second.advance(discovered.revision).is_err(),
                    "another process must not interleave with the native config write"
                );
                Ok(next.clone())
            },
        )
        .unwrap();
    assert_eq!(saved.original_observed, Some(next));
    assert_eq!(
        saved.original_binding.as_deref(),
        Some("a".repeat(64).as_str())
    );
    assert!(!saved.automatic_sync);
    assert_eq!(saved.original.as_ref().unwrap().name, "origin");
}
