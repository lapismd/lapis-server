use super::{BrowserFixture, VERSION_HEADER};
use serde_json::{Value, json};

#[tokio::test]
async fn database_calls_keep_cookie_csrf_session_ownership_and_logout_fencing() {
    let mut f = BrowserFixture::start(true, None).await;
    let (cookie, info, _) = f.login().await;
    let attached = f.attach(&cookie, &info).await;
    let path = format!("{}/api/v3/clients/{}/database", f.base, attached.client_id);
    let input = json!({"method":"setMeta","args":[{"kind":"value","value":"saved"},{"kind":"value","value":true}]});
    for (origin, csrf, version, expected) in [
        (None, None, 3, 403),
        (Some(f.origin), Some("wrong"), 3, 403),
        (
            Some("https://foreign.invalid"),
            Some(info.csrf_token.as_str()),
            3,
            403,
        ),
        (Some(f.origin), Some(info.csrf_token.as_str()), 2, 409),
    ] {
        let mut request = f
            .client
            .post(&path)
            .header("cookie", &cookie)
            .header(VERSION_HEADER, version);
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        if let Some(csrf) = csrf {
            request = request.header("x-lapis-csrf", csrf);
        }
        assert_eq!(
            request.json(&input).send().await.unwrap().status(),
            expected
        );
    }
    let saved = f
        .authorize(f.client.post(&path), &cookie, &info, true)
        .json(&input)
        .send()
        .await
        .unwrap();
    assert_eq!(saved.status(), 200);
    assert_eq!(
        saved.json::<Value>().await.unwrap(),
        json!({"kind":"undefined"})
    );
    let (other, other_info, _) = f.login().await;
    assert_eq!(
        f.authorize(f.client.post(&path), &other, &other_info, true)
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.authorize(
            f.client.post(format!("{}/api/v3/auth/logout", f.base)),
            &cookie,
            &info,
            true
        )
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    assert_eq!(
        f.authorize(f.client.post(&path), &cookie, &info, true)
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        f.authorize(f.client.post(&path), &other, &other_info, true)
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let fresh = f.attach(&other, &other_info).await;
    assert_eq!(fresh.generation, attached.generation);
    let read = f
        .authorize(
            f.client.post(format!(
                "{}/api/v3/clients/{}/database",
                f.base, fresh.client_id
            )),
            &other,
            &other_info,
            true,
        )
        .json(&json!({"method":"getMeta","args":[{"kind":"value","value":"saved"}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(read.status(), 200);
    assert_eq!(
        read.json::<Value>().await.unwrap(),
        json!({"kind":"value","value":true})
    );
    f.finish().await;
}
