use crate::support::Fixture;
use lapis_app_protocol::ClientDescriptor;
use reqwest::RequestBuilder;
use serde_json::{Value, json};

async fn result(f: &Fixture, attached: &ClientDescriptor, method: &str, args: Vec<Value>) -> Value {
    let response = request(f, attached, method, args).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    response.json().await.unwrap()
}
async fn finish(f: &Fixture) {
    let host = f.host.clone();
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}

fn request(
    f: &Fixture,
    attached: &ClientDescriptor,
    method: &str,
    args: Vec<Value>,
) -> RequestBuilder {
    f.authorize(f.client.post(format!(
        "{}/api/v3/clients/{}/database",
        f.base, attached.client_id
    )))
    .json(&json!({"method":method,"args":args.into_iter().map(|value| json!({"kind":"value","value":value})).collect::<Vec<_>>()}))
}

#[tokio::test]
async fn native_database_wire_values_use_the_admitted_vault_owner() {
    let f = Fixture::with_database().await;
    let attached = f.attach(f.ids[0]).await;
    let response = request(&f, &attached, "getMeta", vec![json!("missing")])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({"kind":"undefined"})
    );
    let peer = f.attach(f.ids[0]).await;
    let other = f.attach(f.ids[1]).await;
    assert_eq!(
        result(&f, &attached, "setMeta", vec![json!("null"), Value::Null]).await,
        json!({"kind":"undefined"})
    );
    assert_eq!(
        result(&f, &peer, "getMeta", vec![json!("null")]).await,
        json!({"kind":"value","value":null})
    );
    result(&f, &attached, "setMeta", vec![json!("shared"), json!("💎")]).await;
    assert_eq!(
        result(&f, &peer, "getMeta", vec![json!("shared")]).await,
        json!({"kind":"value","value":"💎"})
    );
    assert_eq!(
        result(&f, &other, "getMeta", vec![json!("shared")]).await,
        json!({"kind":"undefined"})
    );
    let directory = f.profile.as_ref().unwrap().path().join("databases");
    assert_eq!(
        lapis_app_core::database::Database::open(&directory, f.ids[0])
            .await
            .err()
            .unwrap()
            .code(),
        lapis_app_protocol::ErrorCode::Busy
    );
    f.detach(&attached).await;
    f.detach(&peer).await;
    let reconnect = f.attach(f.ids[0]).await;
    assert_eq!(
        result(&f, &reconnect, "getMeta", vec![json!("shared")]).await,
        json!({"kind":"value","value":"💎"})
    );
    finish(&f).await;
}

#[tokio::test]
async fn database_routes_keep_authentication_protocol_origin_and_method_admission() {
    let f = Fixture::with_database().await;
    let attached = f.attach(f.ids[0]).await;
    let url = format!("{}/api/v3/clients/{}/database", f.base, attached.client_id);
    let input = json!({"method":"getMeta","args":[{"kind":"value","value":"private"}]});
    for (token, version, origin, expected) in [
        (None, Some("3"), None, 401),
        (Some(crate::support::TOKEN), None, None, 409),
        (Some(crate::support::TOKEN), Some("2"), None, 409),
        (
            Some(crate::support::TOKEN),
            Some("3"),
            Some("https://foreign.invalid"),
            403,
        ),
    ] {
        let mut request = f.client.post(&url);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        if let Some(version) = version {
            request = request.header(lapis_server::http::VERSION_HEADER, version);
        }
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        assert_eq!(
            request.json(&input).send().await.unwrap().status(),
            expected
        );
    }
    for (input, expected) in [
        (
            json!({"method":"execute","args":[{"kind":"value","value":"DROP TABLE app_meta"}]}),
            422,
        ),
        (
            json!({"method":"getMeta","args":[],"root":"/private/root"}),
            422,
        ),
        (
            json!({"method":"getMeta","args":[],"databasePath":"/private/auth.db"}),
            422,
        ),
        (
            json!({"method":"getMeta","args":vec![json!({"kind":"value","value":"key"});5]}),
            400,
        ),
    ] {
        let response = f
            .authorize(f.client.post(&url))
            .json(&input)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        let text = response.text().await.unwrap();
        assert_eq!(
            serde_json::from_str::<lapis_app_protocol::ServiceError>(&text)
                .unwrap()
                .code,
            lapis_app_protocol::ErrorCode::InvalidRequest
        );
        assert!(!text.contains(f.profile.as_ref().unwrap().path().to_str().unwrap()));
    }
    let unknown = ClientDescriptor {
        client_id: uuid::Uuid::new_v4(),
        ..attached
    };
    assert_eq!(
        request(&f, &unknown, "getMeta", vec![json!("key")])
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    finish(&f).await;
}

#[tokio::test]
async fn detached_or_closed_http_clients_cannot_resume_a_database_call() {
    let f = Fixture::with_database().await;
    let attached = f.attach(f.ids[0]).await;
    result(&f, &attached, "setMeta", vec![json!("saved"), json!(true)]).await;
    f.detach(&attached).await;
    assert_eq!(
        request(&f, &attached, "getMeta", vec![json!("saved")])
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let reattached = f.attach(f.ids[0]).await;
    assert_eq!(
        result(&f, &reattached, "getMeta", vec![json!("saved")]).await,
        json!({"kind":"value","value":true})
    );
    assert_eq!(
        f.authorize(
            f.client
                .delete(format!("{}/api/v3/vaults/{}", f.base, f.ids[0]))
        )
        .send()
        .await
        .unwrap()
        .status(),
        204
    );
    assert_eq!(
        request(&f, &reattached, "getMeta", vec![json!("saved")])
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let db = lapis_app_core::database::Database::open(
        &f.profile.as_ref().unwrap().path().join("databases"),
        f.ids[0],
    )
    .await
    .unwrap();
    assert_eq!(db.get_meta("saved").await.unwrap(), Some(json!(true)));
    db.close().await.unwrap();
    finish(&f).await;
}

#[tokio::test]
async fn database_json_has_its_own_bound_without_raising_the_file_command_limit() {
    let f = Fixture::with_database().await;
    let attached = f.attach(f.ids[0]).await;
    let data = "x".repeat(3 * 1024 * 1024);
    result(&f, &attached, "setMeta", vec![json!("large"), json!(data)]).await;
    let stored = result(&f, &attached, "getMeta", vec![json!("large")]).await;
    assert_eq!(stored["value"].as_str().unwrap().len(), data.len());
    let rejected = request(
        &f,
        &attached,
        "setMeta",
        vec![json!("rejected"), json!("x".repeat(33 * 1024 * 1024))],
    )
    .send()
    .await
    .unwrap();
    assert_eq!(rejected.status(), 413);
    assert_eq!(
        rejected
            .headers()
            .get("connection")
            .and_then(|value| value.to_str().ok()),
        Some("close")
    );
    assert_eq!(
        result(&f, &attached, "getMeta", vec![json!("rejected")]).await,
        json!({"kind":"undefined"})
    );
    let response = f
        .authorize(f.client.post(format!(
            "{}/api/v3/clients/{}/commands",
            f.base, attached.client_id
        )))
        .json(&json!({"untrusted":data}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
    assert_eq!(
        response
            .headers()
            .get("connection")
            .and_then(|value| value.to_str().ok()),
        Some("close")
    );
    finish(&f).await;
}

#[tokio::test]
async fn unselected_database_is_explicitly_unavailable_without_a_fallback() {
    let f = Fixture::start().await;
    let attached = f.attach(f.ids[0]).await;
    assert_eq!(
        request(&f, &attached, "getMeta", vec![json!("key")])
            .send()
            .await
            .unwrap()
            .status(),
        501
    );
    assert!(
        f.roots
            .iter()
            .all(|root| std::fs::read_dir(root.path()).unwrap().count() == 0)
    );
    finish(&f).await;
}

#[cfg(unix)]
#[test]
fn server_database_and_model_storage_cannot_overlap_vault_content_or_symlinks() {
    use lapis_server::{
        config::{Config, VaultConfig},
        host::Host,
    };
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let profile = root.path().join("profile");
    let vault = root.path().join("vault");
    std::fs::create_dir(&profile).unwrap();
    std::fs::create_dir(&vault).unwrap();
    let config = |path: &std::path::Path| Config {
        vaults: vec![VaultConfig {
            id: uuid::Uuid::new_v4(),
            name: "Notes".into(),
            root: path.into(),
        }],
    };
    assert!(Host::with_profile(config(root.path()), &profile).is_err());
    assert_eq!(std::fs::read_dir(&profile).unwrap().count(), 0);
    let models = profile.join("models");
    std::fs::create_dir(&models).unwrap();
    assert!(Host::with_profile(config(&models), &profile).is_err());
    std::fs::remove_dir(&models).unwrap();
    symlink(&vault, &models).unwrap();
    assert!(Host::with_profile(config(&vault), &profile).is_err());
    assert_eq!(std::fs::read_dir(&vault).unwrap().count(), 0);
}
