mod credentials;
#[path = "support/native_participants/mod.rs"]
#[allow(dead_code)]
mod support;
use lapis_app_protocol::participants::{PARTICIPANT_RECORD_LIMIT, PARTICIPANT_REQUEST_LIMIT};
use lapis_server::http::VERSION_HEADER;
use serde_json::{Value, json};
use std::fs;
use support::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn participant_http_reuses_principal_origin_csrf_version_and_strict_frames() {
    let f = Fixture::start(true, false).await;
    let client = f.attach(0).await;
    let path = format!("/api/v3/clients/{}/participants", client.client_id);
    let value = json!({"kind":"read","acceptance_id":uuid::Uuid::new_v4()});
    assert_eq!(
        f.raw(&path).json(&value).send().await.unwrap().status(),
        401
    );
    let (cookie, session) = f.cookie.as_ref().unwrap();
    for (origin, csrf, version, status) in [
        (ORIGIN, "wrong", 3, 403),
        (
            "http://foreign.invalid",
            session.csrf_token.as_str(),
            3,
            403,
        ),
        (ORIGIN, session.csrf_token.as_str(), 2, 409),
    ] {
        assert_eq!(
            f.raw(&path)
                .header("cookie", cookie)
                .header("origin", origin)
                .header("x-lapis-csrf", csrf)
                .header(VERSION_HEADER, version)
                .json(&value)
                .send()
                .await
                .unwrap()
                .status(),
            status
        );
    }
    let (other, info) = f.login().await;
    assert_eq!(
        f.raw(&path)
            .header("cookie", other)
            .header("origin", ORIGIN)
            .header("x-lapis-csrf", info.csrf_token)
            .header(VERSION_HEADER, 3)
            .json(&value)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.raw(&path)
            .bearer_auth(TOKEN)
            .header(VERSION_HEADER, 3)
            .json(&value)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        command(&f, client.client_id, value.clone())
            .send()
            .await
            .unwrap()
            .status(),
        501
    );
    for key in ["vault_id", "executable", "environment", "resume", "root"] {
        let mut forged = value.clone();
        forged[key] = json!("foreign");
        assert_eq!(
            command(&f, client.client_id, forged)
                .send()
                .await
                .unwrap()
                .status(),
            422
        );
    }
    let missing = json!({"kind":"cancel","acceptance_id":uuid::Uuid::new_v4()});
    assert_eq!(
        command(&f, client.client_id, missing)
            .send()
            .await
            .unwrap()
            .status(),
        422
    );
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn participant_http_preserves_escaped_capture_and_enforces_complete_record_bounds() {
    let f = Fixture::start(false, true).await;
    peer(&f);
    let client = f.attach(0).await;
    let mut input = input(4);
    input["user"]["text"] = json!("\"".repeat(32768));
    let text = input["user"]["text"].clone();
    for turn in input["turns"].as_array_mut().unwrap() {
        turn["prompt"] = text.clone();
    }
    let body = serde_json::to_string(&json!({"kind":"accept","input":input}))
        .unwrap()
        .replace("\\\"", "\\u0022");
    assert!(body.len() < PARTICIPANT_REQUEST_LIMIT);
    let response = f
        .auth(f.raw(&format!(
            "/api/v3/clients/{}/participants",
            client.client_id
        )))
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let message: Value = response.json().await.unwrap();
    assert_eq!(message["message"]["input"], input);
    release(&f);
    f.until(|| terminal(&f, &input["acceptance_id"])).await;
    let read = json!({"kind":"read","acceptance_id":input["acceptance_id"]});
    let path = f
        .profile
        .join("turns/participants")
        .join(f.ids[0].to_string())
        .join(format!("{}.json", input["acceptance_id"].as_str().unwrap()));
    let original = fs::read(&path).unwrap();
    let mut exact = original.clone();
    exact.resize(PARTICIPANT_RECORD_LIMIT, b' ');
    fs::write(&path, &exact).unwrap();
    let bytes = command(&f, client.client_id, read.clone())
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert!(bytes.len() <= PARTICIPANT_RECORD_LIMIT);
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["message"]["input"], input);
    exact.push(b' ');
    fs::write(&path, &exact).unwrap();
    let denied = command(&f, client.client_id, read).send().await.unwrap();
    assert_eq!(denied.status(), 400);
    assert_eq!(fs::read(&path).unwrap(), exact);
    fs::write(path, original).unwrap();
    let too_large = f
        .auth(f.raw(&format!(
            "/api/v3/clients/{}/participants",
            client.client_id
        )))
        .header("content-type", "application/json")
        .body(" ".repeat(PARTICIPANT_REQUEST_LIMIT + 1))
        .send()
        .await
        .unwrap();
    assert_eq!(too_large.status(), 413);
    assert_eq!(f.prompts(), 4);
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn participant_http_refuses_forged_rich_or_runtime_capture_and_isolates_other_vault() {
    let f = Fixture::start(false, true).await;
    peer(&f);
    let client = f.attach(0).await;
    for key in ["executable", "arguments", "environment", "root", "resume"] {
        let mut input = input(2);
        input[key] = json!("foreign");
        assert_eq!(
            command(&f, client.client_id, json!({"kind":"accept","input":input}))
                .send()
                .await
                .unwrap()
                .status(),
            422
        );
    }
    let mut rich = input(2);
    rich["turns"][0]["context"] = json!("unsupported rich context");
    assert_eq!(
        command(&f, client.client_id, json!({"kind":"accept","input":rich}))
            .send()
            .await
            .unwrap()
            .status(),
        501
    );
    let mut target = input(2);
    target["turns"][0]["target_id"] =
        target["dispatch"]["deliveries"][0]["recipient"]["id"].clone();
    assert_eq!(
        command(
            &f,
            client.client_id,
            json!({"kind":"accept","input":target})
        )
        .send()
        .await
        .unwrap()
        .status(),
        501
    );
    let mut invalid = input(2);
    invalid["dispatch"]["deliveries"][0]["recipient"]["id"] = json!("foreign");
    assert_eq!(
        command(
            &f,
            client.client_id,
            json!({"kind":"accept","input":invalid})
        )
        .send()
        .await
        .unwrap()
        .status(),
        400
    );
    assert_eq!(
        command(
            &f,
            client.client_id,
            json!({"kind":"read","acceptance_id":uuid::Uuid::nil()})
        )
        .send()
        .await
        .unwrap()
        .status(),
        400
    );
    let input = input(2);
    call(&f, client.client_id, json!({"kind":"accept","input":input})).await;
    f.until(|| f.prompts() == 2).await;
    let other = f.attach(1).await;
    assert_eq!(
        call(
            &f,
            other.client_id,
            json!({"kind":"read","acceptance_id":input["acceptance_id"]})
        )
        .await,
        json!({"kind":"message","message":null})
    );
    for kind in ["stop", "cancel"] {
        let mut control = json!({"kind":kind,"acceptance_id":input["acceptance_id"]});
        if kind == "cancel" {
            control["request_id"] = input["turns"][0]["request_id"].clone();
        }
        assert_eq!(
            command(&f, other.client_id, control)
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    release(&f);
    f.until(|| terminal(&f, &input["acceptance_id"])).await;
    assert_eq!(f.prompts(), 2);
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn participant_http_revocation_detach_and_close_preserve_accepted_lifetime() {
    let mut f = Fixture::start(true, true).await;
    peer(&f);
    let first = f.attach(0).await;
    let second = f.attach(0).await;
    let input = input(2);
    call(&f, first.client_id, json!({"kind":"accept","input":input})).await;
    f.until(|| f.prompts() == 2).await;
    let read = json!({"kind":"read","acceptance_id":input["acceptance_id"]});
    f.detach(first.client_id).await;
    assert_eq!(
        command(&f, first.client_id, read.clone())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let (logout, observe) = tokio::join!(
        f.auth(f.raw("/api/v3/auth/logout")).send(),
        command(&f, second.client_id, read.clone()).send()
    );
    assert!(logout.unwrap().status().is_success());
    let response = observe.unwrap();
    assert!(matches!(response.status().as_u16(), 200 | 401 | 403));
    if response.status().is_success() {
        let value: Value = response.json().await.unwrap();
        assert_eq!(value["message"]["input"], input);
    }
    assert_eq!(
        command(&f, second.client_id, read.clone())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    f.cookie = Some(f.login().await);
    assert_eq!(
        command(&f, second.client_id, read.clone())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let client = f.attach(0).await;
    assert_eq!(
        call(&f, client.client_id, read.clone()).await["message"]["input"],
        input
    );
    release(&f);
    f.until(|| terminal(&f, &input["acceptance_id"])).await;
    let host = f.host.clone();
    let vault = f.ids[0];
    tokio::task::spawn_blocking(move || host.close(vault))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        command(&f, client.client_id, read)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(f.prompts(), 2);
    f.close().await;
}
