use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};

#[test]
fn storage_refuses_replaced_roots_public_permissions_and_lost_identity() {
    let base = tempfile::tempdir().unwrap();
    let profile = VaultRoot::open(base.path()).unwrap();
    let root = storage::child(&profile, "repository-remotes").unwrap();
    let identity = storage::identity(&root).unwrap();
    assert_eq!(
        storage::identity(&root).unwrap().installation,
        identity.installation
    );
    let secrets = storage::child(&root, "credentials").unwrap();
    std::fs::write(
        secrets.path().join("retained.sealed"),
        b"retained ciphertext",
    )
    .unwrap();
    std::fs::remove_file(root.path().join("identity.json")).unwrap();
    assert!(storage::identity(&root).is_err());
    assert_eq!(
        std::fs::read(secrets.path().join("retained.sealed")).unwrap(),
        b"retained ciphertext"
    );
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(storage::child(&profile, "repository-remotes").is_err());
    let elsewhere = tempfile::tempdir().unwrap();
    symlink(elsewhere.path(), base.path().join("alias")).unwrap();
    assert!(storage::child(&profile, "alias").is_err());
}

#[test]
fn startup_refuses_key_material_in_profile_vault_or_a_symlink_to_either() {
    let base = tempfile::tempdir().unwrap();
    let profile = base.path().join("profile");
    let vault = base.path().join("vault");
    std::fs::create_dir(&profile).unwrap();
    std::fs::create_dir(&vault).unwrap();
    let config = Config {
        vaults: vec![crate::config::VaultConfig {
            id: uuid::Uuid::new_v4(),
            name: "Fixture".into(),
            root: vault.clone(),
        }],
    };
    for path in [profile.join("key"), vault.join("key")] {
        std::fs::write(&path, [3; 32]).unwrap();
        assert!(
            RemoteInstallation {
                key_file: Some(path.clone()),
                ..Default::default()
            }
            .select(&profile, &config)
            .is_err()
        );
        let link = base.path().join("link");
        symlink(&path, &link).unwrap();
        assert!(
            RemoteInstallation {
                key_file: Some(link.clone()),
                ..Default::default()
            }
            .select(&profile, &config)
            .is_err()
        );
        std::fs::remove_file(link).unwrap();
    }
    assert!(
        RemoteInstallation {
            key_file: Some("relative".into()),
            ..Default::default()
        }
        .select(&profile, &config)
        .is_err()
    );
    assert!(
        RemoteInstallation::default()
            .select(&profile, &config)
            .is_ok()
    );
}
