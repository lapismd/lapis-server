use super::{BrowserFixture, PROTOCOL_MAJOR, VERSION_HEADER};
use serde_json::json;

#[tokio::test]
async fn remote_commands_require_current_owner_cookie_origin_and_csrf() {
    let mut f = BrowserFixture::start(true, None).await;
    let (cookie, info, _) = f.login().await;
    let c = f.attach(&cookie, &info).await;
    let url = format!(
        "{}/api/v3/clients/{}/repository-remotes?vault={}&generation={}",
        f.base, c.client_id, c.vault_id, c.generation
    );
    let command = json!({"kind":"fetch", "target":"backup", "expectedRevision":0});
    for (origin, csrf) in [
        (None, None),
        (Some(f.origin), None),
        (
            Some("https://foreign.example"),
            Some(info.csrf_token.as_str()),
        ),
    ] {
        let mut request = f
            .client
            .post(&url)
            .header("cookie", &cookie)
            .header(VERSION_HEADER, PROTOCOL_MAJOR);
        if let Some(value) = origin {
            request = request.header("origin", value);
        }
        if let Some(value) = csrf {
            request = request.header("x-lapis-csrf", value);
        }
        assert_eq!(request.json(&command).send().await.unwrap().status(), 403);
    }
    assert_eq!(
        f.authorize(f.client.post(&url), &cookie, &info, true)
            .json(&command)
            .send()
            .await
            .unwrap()
            .status(),
        501
    );
    f.authorize(
        f.client.post(format!("{}/api/v3/auth/logout", f.base)),
        &cookie,
        &info,
        true,
    )
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap();
    assert_eq!(
        f.authorize(f.client.get(&url), &cookie, &info, false)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    f.finish().await;
}
