use super::*;
use support::OWNER;

#[tokio::test]
async fn owner_and_content_authorities_are_distinct_and_inputs_are_bounded() {
    let mut server = Server::start(false, ORIGIN).await;
    let url = format!("{}/api/v3/content-access", server.base);
    assert_eq!(server.client.get(&url).send().await.unwrap().status(), 401);
    assert_eq!(
        server
            .client
            .post(&url)
            .bearer_auth(OWNER)
            .json(&json!({"enabled":true}))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        server
            .owner(Method::POST, "/api/v3/content-access")
            .header("origin", "https://foreign.example")
            .json(&json!({"enabled":true}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        server
            .owner(Method::POST, "/api/v3/content-access")
            .json(&json!({"enabled":true,"scope":"all"}))
            .send()
            .await
            .unwrap()
            .status(),
        422
    );
    server.enable(true).await;
    let grant = server.issue(&server.ids[..1]).await;
    let token = grant["token"].as_str().unwrap();
    assert_eq!(
        server
            .client
            .get(&url)
            .bearer_auth(token)
            .header("x-lapis-protocol", 3)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        server
            .external("/api/content/v1/capabilities", OWNER)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        server
            .external("/api/content/v1/capabilities", token)
            .header("host", "foreign.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    for input in [
        json!({"label":"empty","vault_ids":[],"lifetime_seconds":60}),
        json!({"label":"duplicate","vault_ids":[server.ids[0],server.ids[0]],"lifetime_seconds":60}),
        json!({"label":"long","vault_ids":[server.ids[0]],"lifetime_seconds":86401}),
        json!({"label":"\n","vault_ids":[server.ids[0]],"lifetime_seconds":60}),
    ] {
        assert_eq!(
            server
                .owner(Method::POST, "/api/v3/content-access/grants")
                .json(&input)
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    assert_eq!(
        server
            .owner(Method::POST, "/api/v3/content-access/grants")
            .json(&json!({"label":"x".repeat(9000),"vault_ids":server.ids,"lifetime_seconds":60}))
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    server.finish().await;
}

#[tokio::test]
async fn browser_settings_require_owner_session_origin_and_csrf() {
    let mut server = Server::start(true, ORIGIN).await;
    let response = server
        .client
        .post(format!("{}/api/v3/auth/login", server.base))
        .header("origin", ORIGIN)
        .header("x-lapis-protocol", 3)
        .json(&json!({"username":"owner","password":crate::credentials::PASSWORD}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let session: Value = response.json().await.unwrap();
    let select = || {
        server
            .client
            .post(format!("{}/api/v3/content-access", server.base))
            .header("cookie", &cookie)
            .header("x-lapis-protocol", 3)
            .json(&json!({"enabled":true}))
    };
    assert_eq!(select().send().await.unwrap().status(), 403);
    assert_eq!(
        select()
            .header("origin", ORIGIN)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        select()
            .header("origin", ORIGIN)
            .header("x-lapis-csrf", session["csrf_token"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let grant = server.issue(&server.ids[..1]).await;
    assert_eq!(
        server
            .external("/api/content/v1/capabilities", "bad")
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        server
            .client
            .get(format!("{}/api/v3/content-access", server.base))
            .header("cookie", &cookie)
            .bearer_auth(grant["token"].as_str().unwrap())
            .header("x-lapis-protocol", 3)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.finish().await;
}

#[tokio::test]
async fn unsupported_content_origin_keeps_existing_owner_api_available() {
    let mut server = Server::start(false, "http://notes.example").await;
    let status = server.status().await;
    assert_eq!(status["available"], false);
    assert_eq!(status["access"]["enabled"], false);
    assert!(status["mcp_url"].is_null());
    assert_eq!(
        server
            .owner(Method::POST, "/api/v3/content-access")
            .json(&json!({"enabled":true}))
            .send()
            .await
            .unwrap()
            .status(),
        501
    );
    assert_eq!(
        server
            .owner(Method::GET, "/api/v3/vaults")
            .send()
            .await
            .unwrap()
            .status(),
        200
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
    server.finish().await;
}

#[tokio::test]
async fn failed_preference_storage_still_disables_and_fences_credentials() {
    let mut server = Server::start(true, ORIGIN).await;
    server.enable(true).await;
    let grant = server.issue(&server.ids[..1]).await;
    server.close_store().await;
    assert_eq!(
        server
            .owner(Method::POST, "/api/v3/content-access")
            .json(&json!({"enabled":false}))
            .send()
            .await
            .unwrap()
            .status(),
        500
    );
    assert_eq!(server.status().await["access"]["enabled"], false);
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
        404
    );
    assert_eq!(
        server
            .owner(Method::POST, "/api/v3/content-access")
            .json(&json!({"enabled":true}))
            .send()
            .await
            .unwrap()
            .status(),
        500
    );
    assert_eq!(server.status().await["access"]["enabled"], false);
    server.finish().await;
}
