use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

const CALLBACK: &str = "http://127.0.0.1:6872/oauth/callback";
const RESOURCE: &str = "https://notes.example/mcp";
const VERIFIER: &str = "an-example-of-a-unique-long-PKCE-verifier-for-testing-only";

fn request(server: &Server, method: Method, path: &str) -> reqwest::RequestBuilder {
    server
        .client
        .request(method, format!("{}{path}", server.base))
        .header("host", "notes.example")
}

fn credentials(server: &Server, fields: &[(&str, &str)]) -> reqwest::RequestBuilder {
    let data: String = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields)
        .finish();
    request(server, Method::POST, "/oauth/token")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(data)
}

async fn sign_in(server: &Server) -> (String, String) {
    let login = request(server, Method::POST, "/api/v3/auth/login")
        .header("origin", ORIGIN)
        .header("x-lapis-protocol", "3")
        .json(&json!({"username":"owner","password":crate::credentials::PASSWORD}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let cookie = login.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let session: Value = login.json().await.unwrap();
    (cookie, session["csrf_token"].as_str().unwrap().into())
}

#[tokio::test]
async fn oidc_free_oauth_consent_pkce_and_rotating_refresh_survive_restart() {
    let server = Server::start(true, ORIGIN).await;
    server.enable(true).await;
    let challenge_response = request(&server, Method::POST, "/mcp").send().await.unwrap();
    assert_eq!(challenge_response.status(), 401);
    let challenge_scope = challenge_response.headers()["www-authenticate"]
        .to_str()
        .unwrap()
        .strip_prefix(
            "Bearer resource_metadata=\"https://notes.example/.well-known/oauth-protected-resource/mcp\", scope=\"",
        )
        .and_then(|scope| scope.strip_suffix('"'))
        .expect("MCP authentication challenge includes a scope");
    let content = server.status().await;
    let operation_scopes = content["operation_permissions"].as_array().unwrap();
    assert!(operation_scopes.contains(&json!("tasks:write")));
    assert!(operation_scopes.contains(&json!("notes:read")));
    assert!(operation_scopes.contains(&json!("notes:write")));
    let mut resource_scopes = vec![json!("mcp:read")];
    resource_scopes.extend_from_slice(operation_scopes);
    assert_eq!(
        json!(challenge_scope.split_ascii_whitespace().collect::<Vec<_>>()),
        json!(resource_scopes)
    );
    let resource: Value = request(
        &server,
        Method::GET,
        "/.well-known/oauth-protected-resource/mcp",
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(resource["resource"], RESOURCE);
    assert_eq!(resource["scopes_supported"], json!(resource_scopes));
    let metadata: Value = request(
        &server,
        Method::GET,
        "/.well-known/oauth-authorization-server",
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(metadata["issuer"], ORIGIN);
    resource_scopes.insert(1, json!("offline_access"));
    assert_eq!(metadata["scopes_supported"], json!(resource_scopes));
    let icon = request(&server, Method::GET, "/icon.png")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(icon.headers()["content-type"], "image/png");
    let icon = icon.bytes().await.unwrap();
    assert_eq!(&icon[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&icon[16..24], &[0, 0, 2, 0, 0, 0, 2, 0]);
    assert_eq!(
        metadata["code_challenge_methods_supported"],
        json!(["S256"])
    );

    let registered: Value = request(&server, Method::POST, "/oauth/register")
        .json(&json!({"client_name":"Test MCP client","redirect_uris":[CALLBACK],"token_endpoint_auth_method":"none"}))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    let client = registered["client_id"].as_str().unwrap();

    let agent = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let requested_scope = format!("{challenge_scope} offline_access");
    let authz = agent
        .get(format!("{}/oauth/authorize", server.base))
        .header("host", "notes.example")
        .query(&[
            ("client_id", client),
            ("redirect_uri", CALLBACK),
            ("response_type", "code"),
            ("resource", RESOURCE),
            ("state", "test-state"),
            ("scope", requested_scope.as_str()),
            ("code_challenge_method", "S256"),
            (
                "code_challenge",
                &URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER)),
            ),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(authz.status(), 303);
    let authorization = authz.headers()["location"]
        .to_str()
        .unwrap()
        .strip_prefix("/?mcp-consent=")
        .unwrap();
    let path = format!("/api/v3/content-access/mcp/consent/{authorization}");
    assert_eq!(
        server
            .owner(Method::GET, &path)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let (cookie, csrf) = sign_in(&server).await;
    let pending: Value = request(&server, Method::GET, &path)
        .header("cookie", &cookie)
        .header("x-lapis-protocol", "3")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(pending["permissions"], json!(operation_scopes));
    let approved: Value = request(&server, Method::POST, &path)
        .header("cookie", &cookie)
        .header("x-lapis-csrf", csrf)
        .header("x-lapis-protocol", "3")
        .header("origin", ORIGIN)
        .json(&json!({"approve":true,"vault_ids":[server.ids[0]],"permissions":["notes:read","notes:write"]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let redirect = url::Url::parse(approved["redirect"].as_str().unwrap()).unwrap();
    let params: std::collections::BTreeMap<_, _> = redirect.query_pairs().collect();
    assert_eq!(params["state"], "test-state");
    assert_eq!(params["iss"], ORIGIN);
    let code = params["code"].as_ref();
    let code_fields = |verifier| {
        [
            ("grant_type", "authorization_code"),
            ("client_id", client),
            ("code", code),
            ("redirect_uri", CALLBACK),
            ("code_verifier", verifier),
            ("resource", RESOURCE),
        ]
    };
    assert_eq!(
        credentials(&server, &code_fields("wrong"))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let pair: Value = credentials(&server, &code_fields(VERIFIER))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(pair["expires_in"], 900);
    assert_eq!(
        pair["scope"],
        "mcp:read offline_access notes:read notes:write"
    );
    assert_eq!(
        credentials(&server, &code_fields(VERIFIER))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let access = pair["access_token"].as_str().unwrap();
    let refresh = pair["refresh_token"].as_str().unwrap();
    let mcp = |message: Value| {
        request(&server, Method::POST, "/mcp")
            .bearer_auth(access)
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-11-25")
            .json(&message)
    };
    let initialized: Value = mcp(json!({
        "jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-11-25",
            "capabilities":{},
            "clientInfo":{"name":"OAuth browser client","version":"1.0.0"}
        }
    }))
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(
        mcp(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .send()
            .await
            .unwrap()
            .status(),
        202
    );
    let listed: Value = mcp(json!({
        "jsonrpc":"2.0","id":2,"method":"tools/list","params":{}
    }))
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    let tools = listed["result"]["tools"].as_array().unwrap();
    for name in [
        "read_note",
        "notes_create",
        "notes_replace",
        "notes_search",
        "notes_frontmatter-get",
        "notes_frontmatter-patch",
    ] {
        assert!(tools.iter().any(|tool| tool["name"] == name), "{listed}");
    }
    assert!(tools.iter().all(|tool| {
        let name = tool["name"].as_str().unwrap();
        !name.starts_with("skills_") && !name.starts_with("tasks_")
    }));
    let capabilities: Value = mcp(json!({
        "jsonrpc":"2.0","id":3,"method":"tools/call","params":{
            "name":"capabilities","arguments":{}
        }
    }))
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    let operations = capabilities["result"]["structuredContent"]["outcome"]["value"]
        ["capabilities"]["operations"]
        .as_array()
        .unwrap();
    assert_eq!(operations.len(), tools.len());
    for (tool, operation) in tools.iter().zip(operations) {
        assert_eq!(tool["name"], operation["operation"]);
        let (effect, scope) = if tool["annotations"]["readOnlyHint"] == true {
            ("read_only", "read")
        } else {
            ("write", "write")
        };
        assert_eq!(operation["effect"], effect);
        assert_eq!(operation["scope"], scope);
    }
    let created: Value = mcp(json!({
        "jsonrpc":"2.0","id":4,"method":"tools/call","params":{
            "name":"notes_create","arguments":{
              "vault_id":server.ids[0],"invocation_id":uuid::Uuid::new_v4(),
              "path":"OAuth.md","text":"# Authorized notes"
            }
        }
    }))
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(
        created["result"]["structuredContent"]["outcome"]["status"], "ok",
        "{created}"
    );
    assert_eq!(
        created["result"]["structuredContent"]["outcome"]["value"]["path"],
        "OAuth.md"
    );
    let version = &created["result"]["structuredContent"]["outcome"]["value"]["version"];
    let patched: Value = mcp(json!({
        "jsonrpc":"2.0","id":5,"method":"tools/call","params":{
            "name":"notes_frontmatter-patch","arguments":{
              "vault_id":server.ids[0],"invocation_id":uuid::Uuid::new_v4(),
              "path":"OAuth.md","expected_version":version,"set":{"status":"approved"}
            }
        }
    }))
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(
        patched["result"]["structuredContent"]["outcome"]["value"]["changed"], true,
        "{patched}"
    );
    let frontmatter: Value = mcp(json!({
        "jsonrpc":"2.0","id":6,"method":"tools/call","params":{
            "name":"notes_frontmatter-get","arguments":{"vault_id":server.ids[0],"path":"OAuth.md"}
        }
    }))
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(
        frontmatter["result"]["structuredContent"]["outcome"]["value"]["properties"],
        json!({"status":"approved"})
    );

    let server = server.restart().await;
    assert_eq!(
        server
            .external("/api/content/v1/vaults", access)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let note: Value = server
        .mcp(access, server.ids[0])
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        note["result"]["structuredContent"]["outcome"]["value"]["text"],
        SAVED
    );

    let initialize = request(&server, Method::POST, "/mcp")
        .bearer_auth(access)
        .header("accept", "application/json, text/event-stream")
        .json(&json!({
            "jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25",
                "capabilities":{},"clientInfo":{"name":"acceptance","version":"1"}},
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(initialize.status(), 200);
    let initialized: Value = initialize.json().await.unwrap();
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(
        initialized["result"]["serverInfo"]["icons"],
        json!([{"src": format!("{ORIGIN}/icon.png"), "mimeType":"image/png", "sizes":["512x512"]}])
    );

    let refresh_fields = [
        ("grant_type", "refresh_token"),
        ("client_id", client),
        ("refresh_token", refresh),
        ("resource", RESOURCE),
    ];
    let rotated: Value = credentials(&server, &refresh_fields)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let rotated_token = rotated["access_token"].as_str().unwrap();
    assert_eq!(
        server
            .mcp(rotated_token, server.ids[0])
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        credentials(&server, &refresh_fields)
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        server
            .mcp(rotated_token, server.ids[0])
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
}

#[tokio::test]
async fn swagger_requires_an_owner_cookie_and_serves_the_generated_routes() {
    let mut server = Server::start(true, ORIGIN).await;
    assert_eq!(
        request(&server, Method::GET, "/api/docs")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        server
            .owner(Method::GET, "/api/openapi.json")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let (cookie, _) = sign_in(&server).await;
    let docs = request(&server, Method::GET, "/api/docs")
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert!(docs.headers().contains_key("content-security-policy"));
    assert!(docs.text().await.unwrap().contains("swagger-ui-bundle.js"));
    let spec: Value = request(&server, Method::GET, "/api/openapi.json")
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(spec["openapi"], "3.1.0");
    assert!(spec["paths"]["/api/v3/content-access/tokens"]["post"]["requestBody"].is_object());
    assert!(spec["paths"]["/mcp"]["post"]["security"].is_array());
    assert!(spec["paths"]["/api/content/v1/vaults/{vault_id}/note"]["get"]["responses"]["200"]["headers"]["ETag"].is_object());
    assert_eq!(
        spec["paths"]["/api/v3/clients/{client}/terminal/{session}/events"]["get"]["responses"]["200"]
            ["content"]["text/event-stream"]["schema"]["type"],
        "string"
    );
    assert!(spec["x-lapis-plugin-operations"].as_array().unwrap().len() > 4);
    server.finish().await;
}
