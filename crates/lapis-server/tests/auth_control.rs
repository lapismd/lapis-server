use lapis_server::control::{Control, LoginError, SESSION_ABSOLUTE_MS, SESSION_IDLE_MS, passwords};

mod credentials;
use credentials::{HASH, NOW, PASSWORD};

#[test]
fn passwords_keep_node_scrypt_format_and_bounded_parameters() {
    assert!(passwords::verify(PASSWORD, HASH));
    assert!(!passwords::verify("incorrect", HASH));
    assert!(!passwords::verify(
        PASSWORD,
        &HASH.replace("131072", "1073741824")
    ));
    assert!(!passwords::verify(PASSWORD, "invalid"));
    assert!(passwords::hash("short").is_err());
    let encoded = passwords::hash("a new sufficiently long password").unwrap();
    assert_ne!(encoded, HASH);
    assert!(passwords::verify(
        "a new sufficiently long password",
        &encoded
    ));
}

#[tokio::test]
async fn session_tokens_are_hashed_persistent_revocable_and_expire_without_long_waits() {
    let root = tempfile::tempdir().unwrap();
    let control = Control::open(root.path()).await.unwrap();
    assert!(control.initialize("Owner", HASH, NOW).await.unwrap());
    assert!(!control.initialize("Different", HASH, NOW).await.unwrap());
    let issued = control.issue(NOW).await.unwrap();
    assert_eq!(issued.session.username, "Owner");
    assert_eq!(issued.session.expires_at, NOW + SESSION_ABSOLUTE_MS);
    let token = issued.token;
    let id = issued.session.id;
    assert!(control.resolve(&token, NOW + 1).await.unwrap().is_some());
    control.close().await.unwrap();
    drop(control);
    let control = Control::open(root.path()).await.unwrap();
    assert!(control.resolve(&token, NOW + 2).await.unwrap().is_some());
    control.revoke(id, NOW + 3).await.unwrap();
    assert!(control.resolve(&token, NOW + 4).await.unwrap().is_none());
    let issued = control.issue(NOW + 5).await.unwrap();
    assert!(
        control
            .resolve(&issued.token, NOW + 5 + SESSION_IDLE_MS)
            .await
            .unwrap()
            .is_none()
    );
    let issued = control.issue(NOW).await.unwrap();
    for days in [6, 12, 18, 24, 29] {
        assert!(
            control
                .resolve(&issued.token, NOW + days * 86_400_000)
                .await
                .unwrap()
                .is_some()
        );
    }
    assert!(
        control
            .resolve(&issued.token, NOW + SESSION_ABSOLUTE_MS)
            .await
            .unwrap()
            .is_none()
    );
    control.close().await.unwrap();
    drop(control);
    let database =
        turso::Builder::new_local(root.path().join("control/auth.turso").to_str().unwrap())
            .build()
            .await
            .unwrap();
    let connection = database.connect().unwrap();
    let mut rows = connection
        .query(
            "SELECT token_hash FROM sessions WHERE id=?",
            [issued.session.id.to_string()],
        )
        .await
        .unwrap();
    let stored = rows
        .next()
        .await
        .unwrap()
        .unwrap()
        .get::<String>(0)
        .unwrap();
    use sha2::{Digest, Sha256};
    assert_eq!(
        stored,
        format!("{:x}", Sha256::digest(issued.token.as_bytes()))
    );
    assert_ne!(stored, issued.token);
}

#[tokio::test]
async fn bootstrap_is_idempotent_and_never_places_credentials_inside_an_admitted_vault() {
    use lapis_server::control::bootstrap::{self, Bootstrap};
    let root = tempfile::tempdir().unwrap();
    let vault = root.path().join("vault");
    std::fs::create_dir(&vault).unwrap();
    let profile = root.path().join("profile");
    let password = root.path().join("password");
    std::fs::write(&password, format!("{PASSWORD}\r\n")).unwrap();
    let control = bootstrap::open(
        &profile,
        std::slice::from_ref(&vault),
        Some(Bootstrap {
            username: "Owner".into(),
            password_file: password.clone(),
        }),
    )
    .await
    .unwrap();
    let issued = control
        .login("owner", PASSWORD, "local", NOW)
        .await
        .unwrap();
    control.close().await.unwrap();
    drop(control);
    std::fs::remove_file(&password).unwrap();
    let control = bootstrap::open(&profile, std::slice::from_ref(&vault), None)
        .await
        .unwrap();
    assert!(
        control
            .resolve(&issued.token, NOW + 1)
            .await
            .unwrap()
            .is_some()
    );
    control.close().await.unwrap();
    assert!(
        bootstrap::open(&vault.join("profile"), std::slice::from_ref(&vault), None)
            .await
            .is_err()
    );
    let exposed = vault.join("password");
    std::fs::write(&exposed, PASSWORD).unwrap();
    assert!(
        bootstrap::open(
            &root.path().join("other-profile"),
            &[vault],
            Some(Bootstrap {
                username: "Owner".into(),
                password_file: exposed
            })
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn failed_login_throttle_and_successful_owner_login_keep_existing_policy() {
    let root = tempfile::tempdir().unwrap();
    let control = Control::open(root.path()).await.unwrap();
    control.initialize("Owner", HASH, NOW).await.unwrap();
    let issued = control
        .login("owner", PASSWORD, "127.0.0.1", NOW)
        .await
        .unwrap();
    assert_eq!(issued.session.username, "Owner");
    let key = Control::login_key("owner", "127.0.0.1");
    for i in 0..8 {
        control.record_failure(&key, NOW + i).await.unwrap();
    }
    assert!(matches!(
        control
            .login("owner", PASSWORD, "127.0.0.1", NOW + 10)
            .await,
        Err(LoginError::Throttled)
    ));
    assert!(matches!(
        control
            .login("owner", "incorrect", "other-peer", NOW + 11)
            .await,
        Err(LoginError::Denied)
    ));
    let issued = control
        .login("OWNER", PASSWORD, "127.0.0.1", NOW + 15 * 60_000 + 10)
        .await
        .unwrap();
    assert!(
        control
            .resolve(&issued.token, NOW + 15 * 60_000 + 11)
            .await
            .unwrap()
            .is_some()
    );
    control.close().await.unwrap();
}
