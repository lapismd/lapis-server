use super::{BrowserFixture, PROTOCOL_MAJOR, VERSION_HEADER};
use crate::credentials::PASSWORD;
use lapis_server::control::{Control, now_ms};

#[tokio::test]
async fn browser_cookies_require_https_or_an_explicit_localhost_development_mode() {
    use lapis_server::http::{Authentication, router};
    let mut f = BrowserFixture::start(true, None).await;
    for (origin, insecure) in [
        ("http://outside.invalid", true),
        ("http://localhost:8787", false),
        ("https://notes.example", true),
        ("http://127.0.0.1:8787", true),
    ] {
        assert!(
            router(
                f.host.clone(),
                Authentication::browser(f.control.clone(), None, insecure).unwrap(),
                origin
            )
            .is_err(),
            "{origin}"
        );
    }
    assert!(
        router(
            f.host.clone(),
            Authentication::browser(f.control.clone(), None, false).unwrap(),
            "https://notes.example"
        )
        .is_ok()
    );
    f.finish().await;
}

#[tokio::test]
async fn login_origin_protocol_shape_and_rate_limit_fail_before_unauthorized_admission() {
    let mut f = BrowserFixture::start(true, None).await;
    let path = format!("{}/api/v3/auth/login", f.base);
    let input = serde_json::json!({"username":"owner","password":PASSWORD});
    assert_eq!(
        f.client
            .post(&path)
            .header(VERSION_HEADER, PROTOCOL_MAJOR)
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.client
            .post(&path)
            .header("origin", "https://foreign.invalid")
            .header(VERSION_HEADER, PROTOCOL_MAJOR)
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.client
            .post(&path)
            .header("origin", f.origin)
            .header(VERSION_HEADER, 2)
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    let mut untrusted = input.clone();
    untrusted["root"] = serde_json::json!("/other-root");
    assert_eq!(
        f.client
            .post(&path)
            .header("origin", f.origin)
            .header(VERSION_HEADER, PROTOCOL_MAJOR)
            .json(&untrusted)
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    for (username, password) in [("owner", "wrong"), ("unknown", PASSWORD)] {
        let response = f
            .client
            .post(&path)
            .header("origin", f.origin)
            .header(VERSION_HEADER, PROTOCOL_MAJOR)
            .json(&serde_json::json!({"username":username,"password":password}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
        assert_eq!(
            response.json::<serde_json::Value>().await.unwrap()["message"],
            "Invalid username or password"
        );
    }
    let now = now_ms().unwrap();
    let key = Control::login_key("owner", "127.0.0.1");
    for i in 0..8 {
        f.control.record_failure(&key, now + i).await.unwrap();
    }
    let response = f
        .client
        .post(&path)
        .header("origin", f.origin)
        .header("x-forwarded-for", "another-peer")
        .header(VERSION_HEADER, PROTOCOL_MAJOR)
        .json(&input)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429);
    assert!(!response.headers().contains_key("set-cookie"));
    f.finish().await;
}

#[tokio::test]
async fn cookie_mutations_require_csrf_and_origin_and_never_fall_back_from_invalid_bearer() {
    let mut f = BrowserFixture::start(true, None).await;
    let (cookie, info, _) = f.login().await;
    let path = format!("{}/api/v3/vaults/{}/clients", f.base, f.id);
    let input = serde_json::json!({"protocol_version":PROTOCOL_MAJOR});
    for (origin, csrf) in [
        (None, None),
        (Some(f.origin), None),
        (None, Some(info.csrf_token.as_str())),
        (Some(f.origin), Some("wrong")),
        (
            Some("https://foreign.invalid"),
            Some(info.csrf_token.as_str()),
        ),
    ] {
        let mut request = f
            .client
            .post(&path)
            .header("cookie", &cookie)
            .header(VERSION_HEADER, PROTOCOL_MAJOR)
            .json(&input);
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        if let Some(csrf) = csrf {
            request = request.header("x-lapis-csrf", csrf);
        }
        assert_eq!(request.send().await.unwrap().status(), 403);
    }
    let invalid = f
        .authorize(
            f.client.get(format!("{}/api/v3/vaults", f.base)),
            &cookie,
            &info,
            false,
        )
        .header("authorization", "Bearer incorrect")
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 401);
    let foreign_read = f
        .authorize(
            f.client.get(format!("{}/api/v3/vaults", f.base)),
            &cookie,
            &info,
            false,
        )
        .header("origin", "https://foreign.invalid")
        .send()
        .await
        .unwrap();
    assert_eq!(foreign_read.status(), 403);
    let valid = f
        .authorize(
            f.client.get(format!("{}/api/v3/vaults", f.base)),
            &cookie,
            &info,
            false,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(valid.status(), 200);
    assert!(
        !valid
            .text()
            .await
            .unwrap()
            .contains(f.vault.to_str().unwrap())
    );
    f.finish().await;
}
