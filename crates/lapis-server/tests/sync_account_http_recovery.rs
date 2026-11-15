mod credentials;
#[path = "browser_auth/support.rs"]
mod support;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use support::BrowserFixture;
use uuid::Uuid;
const PROOF: &str = "native-http-recovery-proof-0123456789012345678";
#[tokio::test]
async fn native_outcome_routes_refuse_ambient_authority_and_cancel_only_the_proven_handoff() {
    let mut f = BrowserFixture::start(true, None).await;
    let id=f.client.post(format!("{}/api/v3/sync/account",f.base)).header("x-lapis-protocol","3")
        .json(&json!({"client_id":Uuid::new_v4(),"client_name":"Desktop","s256_challenge":URL_SAFE_NO_PAD.encode(Sha256::digest(PROOF))}))
        .send().await.unwrap().json::<Value>().await.unwrap()["id"].as_str().unwrap().to_owned();
    let request = |action: &str| {
        f.client
            .post(format!("{}/api/v3/sync/account/{id}/{action}", f.base))
            .header("x-lapis-protocol", "3")
    };
    for action in ["inspect", "cancel"] {
        assert_eq!(
            request(action)
                .header("cookie", "ambient=private")
                .json(&json!({"verifier":PROOF}))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
        assert_eq!(
            request(action)
                .bearer_auth("ambient-private")
                .json(&json!({"verifier":PROOF}))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
        assert_eq!(
            request(action)
                .header("origin", "https://foreign.example")
                .json(&json!({"verifier":PROOF}))
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            request(action)
                .body("x".repeat(4097))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
        assert_eq!(
            request(action)
                .json(&json!({"verifier":"wrong-proof-01234567890123456789012345678901234"}))
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
    let (cookie, session, _) = f.login().await;
    let response = f
        .authorize(
            f.client
                .post(format!("{}/api/v3/sync/account/{id}/decision", f.base)),
            &cookie,
            &session,
            true,
        )
        .json(&json!({"approve":true}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let issued = request("redeem")
        .json(&json!({"verifier":PROOF}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let response = request("inspect")
        .json(&json!({"verifier":PROOF}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let text = response.text().await.unwrap();
    assert!(!text.contains(PROOF));
    assert!(!text.contains(issued["token"].as_str().unwrap()));
    assert!(!text.contains("s256"));
    let observed: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(observed["status"], "consumed");
    assert_eq!(observed["credential_status"], "active");
    let mut fields = observed
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "client_id",
            "connection_id",
            "credential_status",
            "expires_at",
            "status"
        ]
    );
    let cancelled = request("cancel")
        .json(&json!({"verifier":PROOF}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(cancelled["credential_status"], "revoked");
    assert_eq!(
        request("cancel")
            .json(&json!({"verifier":PROOF}))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap(),
        cancelled
    );
    assert_eq!(
        f.client
            .get(format!("{}/api/v3/sync/discovery/vaults", f.base))
            .header("x-lapis-protocol", "3")
            .bearer_auth(issued["token"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        request("redeem")
            .json(&json!({"verifier":PROOF}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    std::fs::write(f.vault.join("preserved.md"), "owner draft remains").unwrap();
    let attached = f.attach(&cookie, &session).await;
    assert_eq!(attached.vault_id, f.id);
    let content = f
        .command(
            &cookie,
            &session,
            &attached,
            lapis_app_protocol::VaultCommand::ReadText {
                path: "preserved.md".into(),
            },
        )
        .send()
        .await
        .unwrap();
    assert_eq!(content.status(), 200);
    assert!(
        content
            .text()
            .await
            .unwrap()
            .contains("owner draft remains")
    );
    f.finish().await;
}
