mod credentials;
#[path = "browser_auth/support.rs"]
mod support;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use support::BrowserFixture;
use uuid::Uuid;

const VERIFIER: &str = "native-account-verifier-01234567890123456789012";
const OWNER_TOKEN: &str = "isolated-sync-handoff-owner-token-0123456789";

#[tokio::test]
async fn native_exchange_requires_browser_consent_and_never_grants_vault_authority() {
    let mut f = BrowserFixture::start(true, Some(OWNER_TOKEN)).await;
    let create = f
        .client
        .post(format!("{}/api/v3/sync/account", f.base))
        .header("x-lapis-protocol", "3")
        .json(&json!({"client_id":Uuid::new_v4(),"client_name":"Desktop",
            "s256_challenge":URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER))}));
    let response = create.send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let connection: Value = response.json().await.unwrap();
    let id = connection["id"].as_str().unwrap();
    let redeem = || {
        f.client
            .post(format!("{}/api/v3/sync/account/{id}/redeem", f.base))
            .header("x-lapis-protocol", "3")
            .json(&json!({"verifier":VERIFIER}))
    };
    assert_eq!(redeem().send().await.unwrap().status(), 403);
    let decision = || {
        f.client
            .post(format!("{}/api/v3/sync/account/{id}/decision", f.base))
    };
    assert_eq!(
        decision()
            .json(&json!({"approve":true}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let (cookie, session, _) = f.login().await;
    assert_eq!(
        decision()
            .header("x-lapis-protocol", "3")
            .bearer_auth(OWNER_TOKEN)
            .json(&json!({"approve":true}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let inspect = f
        .authorize(
            f.client.get(format!("{}/api/v3/sync/account/{id}", f.base)),
            &cookie,
            &session,
            false,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(inspect.status(), 200);
    let metadata: Value = inspect.json().await.unwrap();
    assert_eq!(metadata["client_name"], "Desktop");
    assert_eq!(metadata["status"], "pending");
    assert!(metadata.get("s256_challenge").is_none());
    assert_eq!(
        f.authorize(decision(), &cookie, &session, false)
            .json(&json!({"approve":true}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.authorize(decision(), &cookie, &session, true)
            .json(&json!({"approve":true}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let response = redeem().send().await.unwrap();
    assert_eq!(response.status(), 200);
    let issued: Value = response.json().await.unwrap();
    assert_eq!(issued["scope"], "discovery");
    assert_eq!(redeem().send().await.unwrap().status(), 403);
    assert_eq!(
        f.client
            .get(format!("{}/api/v3/vaults", f.base))
            .header("x-lapis-protocol", "3")
            .bearer_auth(issued["token"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    // The owner's existing admitted writer remains usable; account handoff
    // neither replaces the owner session nor changes working-vault authority.
    std::fs::write(f.vault.join("owner.md"), "owner content").unwrap();
    let attached = f.attach(&cookie, &session).await;
    let response = f
        .command(
            &cookie,
            &session,
            &attached,
            lapis_app_protocol::VaultCommand::ReadText {
                path: "owner.md".into(),
            },
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.text().await.unwrap().contains("owner content"));
    f.host
        .enable_management(
            f.control.clone(),
            &f.vault.parent().unwrap().join("profile"),
        )
        .await
        .unwrap();
    let response = f
        .authorize(
            f.client.post(format!("{}/api/v3/vaults", f.base)),
            &cookie,
            &session,
            true,
        )
        .json(&json!({"name":"Managed Sync Vault"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let managed: Value = response.json().await.unwrap();
    let discover = || {
        f.client
            .get(format!("{}/api/v3/sync/discovery/vaults", f.base))
            .header("x-lapis-protocol", "3")
            .bearer_auth(issued["token"].as_str().unwrap())
    };
    let response = discover().send().await.unwrap();
    assert_eq!(response.status(), 200);
    let listing: Value = response.json().await.unwrap();
    assert_eq!(
        listing["vaults"],
        json!([{"vault_id":managed["vault_id"],"name":"Managed Sync Vault"}])
    );
    assert!(!listing.to_string().contains("profile"));
    let response = f
        .client
        .post(format!("{}/api/v3/sync/discovery/sign-out", f.base))
        .header("x-lapis-protocol", "3")
        .bearer_auth(issued["token"].as_str().unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(discover().send().await.unwrap().status(), 401);
    f.finish().await;
}

#[tokio::test]
async fn foreign_origin_ambient_credentials_and_oversized_requests_are_rejected() {
    let mut f = BrowserFixture::start(true, None).await;
    let create = || {
        f.client
            .post(format!("{}/api/v3/sync/account", f.base))
            .header("x-lapis-protocol", "3")
    };
    assert_eq!(
        create()
            .header("origin", "https://foreign.example")
            .body("{}")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        create()
            .header("cookie", "ambient=secret")
            .body("{}")
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        create()
            .body("x".repeat(4097))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    f.finish().await;
}
