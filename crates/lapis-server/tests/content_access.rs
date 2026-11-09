mod credentials;
#[path = "content_access/security.rs"]
mod security;
#[path = "content_access/support.rs"]
mod support;
use reqwest::Method;
use serde_json::{Value, json};
use support::{ORIGIN, SAVED, Server};

#[tokio::test]
async fn owner_controls_enable_scoped_http_and_mcp_and_disable_old_credentials() {
    let mut server = Server::start(false, ORIGIN).await;
    let status = server.status().await;
    assert_eq!(status["access"]["enabled"], false);
    assert_eq!(status["persistent"], false);
    assert_eq!(status["mcp_url"], format!("{ORIGIN}/mcp"));
    assert_eq!(
        server
            .external("/api/content/v1/capabilities", "bad")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        server
            .mcp("bad", server.ids[0])
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    server.enable(true).await;
    let first = server.issue(&server.ids[..1]).await;
    let token = first["token"].as_str().unwrap();
    let status = server.status().await;
    assert!(!status.to_string().contains(token));
    assert_eq!(
        status["access"]["grants"][0]["vault_ids"],
        json!([server.ids[0]])
    );
    assert_eq!(status["access"]["grants"][0]["scope"], "read");
    let path = format!("/api/content/v1/vaults/{}/note?path=note.md", server.ids[0]);
    let response = server.external(&path, token).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers().contains_key("etag"));
    assert_eq!(response.text().await.unwrap(), SAVED);
    let response: Value = server
        .mcp(token, server.ids[0])
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["value"]["text"],
        SAVED
    );
    let foreign = format!("/api/content/v1/vaults/{}/note?path=note.md", server.ids[1]);
    assert_eq!(
        server
            .external(&foreign, token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let response: Value = server
        .mcp(token, server.ids[1])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        response["result"]["structuredContent"]["outcome"]["error"]["code"],
        "PERMISSION_DENIED"
    );
    let second = server.issue(&server.ids[..1]).await;
    let second_token = second["token"].as_str().unwrap();
    let response = server
        .owner(
            Method::DELETE,
            &format!(
                "/api/v3/content-access/grants/{}",
                first["grant_id"].as_str().unwrap()
            ),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    assert_eq!(
        server.external(&path, token).send().await.unwrap().status(),
        401
    );
    assert_eq!(
        server
            .external(&path, second_token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    server.enable(false).await;
    assert!(
        server.status().await["access"]["grants"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        server
            .external(&path, second_token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        server
            .mcp(second_token, server.ids[0])
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    server.enable(true).await;
    assert_eq!(
        server
            .external(&path, second_token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.finish().await;
}

#[tokio::test]
async fn preference_survives_restart_but_credentials_never_do() {
    let server = Server::start(true, ORIGIN).await;
    server.enable(true).await;
    let grant = server.issue(&server.ids[..1]).await;
    let server = server.restart().await;
    assert_eq!(server.status().await["access"]["enabled"], true);
    assert!(
        server.status().await["access"]["grants"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        server
            .external(
                "/api/content/v1/capabilities",
                grant["token"].as_str().unwrap()
            )
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.enable(false).await;
    let mut server = server.restart().await;
    assert_eq!(server.status().await["access"]["enabled"], false);
    server.finish().await;
}

#[tokio::test]
async fn archive_and_native_close_retire_whole_grants_without_reviving_on_restore() {
    let mut server = Server::start(true, ORIGIN).await;
    server.enable(true).await;
    let mixed = server.issue(&server.ids).await;
    let other = server.issue(&server.ids[1..]).await;
    let archive = format!("/api/v3/vaults/{}/archive", server.ids[0]);
    assert_eq!(
        server
            .owner(Method::POST, &archive)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        server
            .external(
                "/api/content/v1/capabilities",
                mixed["token"].as_str().unwrap()
            )
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        server
            .external(
                "/api/content/v1/capabilities",
                other["token"].as_str().unwrap()
            )
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        server
            .owner(Method::POST, "/api/v3/content-access/grants")
            .json(&json!({"label":"archived","vault_ids":[server.ids[0]],"lifetime_seconds":60}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let restore = format!("/api/v3/vaults/{}/restore", server.ids[0]);
    assert_eq!(
        server
            .owner(Method::POST, &restore)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        server
            .external(
                "/api/content/v1/capabilities",
                mixed["token"].as_str().unwrap()
            )
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let close = format!("/api/v3/vaults/{}", server.ids[1]);
    assert_eq!(
        server
            .owner(Method::DELETE, &close)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert!(
        server.status().await["access"]["grants"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    server.finish().await;
}
