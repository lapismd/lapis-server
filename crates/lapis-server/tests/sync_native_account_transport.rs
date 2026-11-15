mod credentials;
use credentials::{HASH, NOW, PASSWORD};
use lapis_app_core::sync::account::{AccountEndpoint, AccountTransportPolicy, AccountVerifier};
use lapis_server::{
    config::Config,
    control::Control,
    host::Host,
    http::{Authentication, router},
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_client_completes_owner_approved_discovery_and_sign_out_against_current_server() {
    let root = tempfile::tempdir().unwrap();
    let profile = root.path().join("profile");
    std::fs::create_dir(&profile).unwrap();
    let host = Arc::new(Host::with_profile(Config { vaults: vec![] }, &profile).unwrap());
    let control = Arc::new(Control::open(&profile).await.unwrap());
    control.initialize("Owner", HASH, NOW).await.unwrap();
    host.enable_management(control.clone(), &profile)
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://localhost:{}", listener.local_addr().unwrap().port());
    let app = router(
        host.clone(),
        Authentication::browser(control.clone(), None, true).unwrap(),
        &origin,
    )
    .unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let base = origin.clone();
    let device = Uuid::new_v4();
    let (native, verifier, connection) = tokio::task::spawn_blocking(move || {
        let native =
            AccountEndpoint::open(&base, AccountTransportPolicy::LoopbackDevelopment).unwrap();
        let verifier = AccountVerifier::generate().unwrap();
        let connection = native
            .initiate(device, "Independent Desktop", &verifier, now())
            .unwrap();
        assert_eq!(connection.client_id, device);
        assert_eq!(
            connection.approval_url,
            format!("{base}/?sync-connect={}", connection.id)
        );
        assert!(
            native
                .redeem(connection.id, device, &verifier, now())
                .is_err()
        );
        (native, verifier, connection)
    })
    .await
    .unwrap();
    let client = reqwest::Client::new();
    let login = client
        .post(format!("{origin}/api/v3/auth/login"))
        .header("origin", &origin)
        .header("x-lapis-protocol", "3")
        .json(&json!({"username":"owner","password":PASSWORD}))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), 200);
    let cookie = login.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let session: Value = login.json().await.unwrap();
    let owner = |path: String| {
        client
            .post(format!("{origin}{path}"))
            .header("origin", &origin)
            .header("x-lapis-protocol", "3")
            .header("cookie", &cookie)
            .header("x-lapis-csrf", session["csrf_token"].as_str().unwrap())
    };
    assert_eq!(
        owner(format!("/api/v3/sync/account/{}/decision", connection.id))
            .json(&json!({"approve":true}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let managed = owner("/api/v3/vaults".into())
        .json(&json!({"name":"Native discovered vault"}))
        .send()
        .await
        .unwrap();
    assert_eq!(managed.status(), 201);
    let managed: Value = managed.json().await.unwrap();
    let expected: Uuid = serde_json::from_value(managed["vault_id"].clone()).unwrap();
    let base = origin.clone();
    let connection_id = connection.id;
    let (native, credential, verifier) = tokio::task::spawn_blocking(move || {
        let issued = native
            .redeem(connection.id, device, &verifier, now())
            .unwrap();
        assert_eq!(issued.client_id, device);
        // Restore from host-owned protected fields, without any browser credential.
        let restored = native
            .restore_credential(
                &base,
                issued.id,
                issued.client_id,
                issued.expires_at,
                issued.expose_secret().to_owned(),
            )
            .unwrap();
        let vaults = native.discover(&restored, now()).unwrap();
        assert_eq!(vaults.len(), 1);
        assert_eq!(vaults[0].vault_id, expected);
        assert_eq!(vaults[0].name, "Native discovered vault");
        (native, restored, verifier)
    })
    .await
    .unwrap();
    // Even this usable native credential cannot enter ordinary working-vault routes.
    let response = client
        .get(format!("{origin}/api/v3/vaults"))
        .header("x-lapis-protocol", "3")
        .bearer_auth(credential.expose_secret())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    tokio::task::spawn_blocking(move || {
        let inspected = native
            .inspect_handoff(connection_id, device, &verifier)
            .unwrap();
        assert_eq!(
            inspected.credential_status,
            lapis_app_core::sync::account::DiscoveryCredentialStatus::Active
        );
        native.sign_out(&credential, now()).unwrap();
        let revoked = native
            .inspect_handoff(connection_id, device, &verifier)
            .unwrap();
        assert_eq!(
            revoked.credential_status,
            lapis_app_core::sync::account::DiscoveryCredentialStatus::Revoked
        );
        assert_eq!(
            native
                .cancel_handoff(connection_id, device, &verifier)
                .unwrap(),
            revoked
        );
        assert!(native.discover(&credential, now()).is_err());
    })
    .await
    .unwrap();
    server.abort();
    let _ = server.await;
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
    control.close().await.unwrap();
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
