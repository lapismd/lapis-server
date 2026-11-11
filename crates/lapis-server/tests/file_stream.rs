#[path = "browser_auth/support.rs"]
#[allow(dead_code)]
mod browser;
#[allow(dead_code)]
mod credentials;
#[allow(dead_code)]
mod support;
use lapis_app_protocol::{ClientDescriptor, file_changes::FileChangeSet};
use std::time::Duration;
use support::Fixture;

fn path(base: &str, client: &ClientDescriptor) -> String {
    format!(
        "{base}/api/v3/clients/{}/file-events?vault_id={}&generation={}",
        client.client_id, client.vault_id, client.generation
    )
}
struct Frames {
    response: reqwest::Response,
    buffer: String,
}
impl Frames {
    async fn next(&mut self) -> Option<(String, FileChangeSet)> {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                while let Some(end) = self.buffer.find("\n\n") {
                    let frame = self.buffer[..end].to_owned();
                    self.buffer.drain(..end + 2);
                    let event = frame.lines().find_map(|line| line.strip_prefix("event: "));
                    let data = frame.lines().find_map(|line| line.strip_prefix("data: "));
                    if let (Some(event), Some(data)) = (event, data) {
                        return Some((event.to_owned(), serde_json::from_str(data).unwrap()));
                    }
                }
                let bytes = self.response.chunk().await.unwrap()?;
                self.buffer.push_str(std::str::from_utf8(&bytes).unwrap());
            }
        })
        .await
        .expect("bounded native observation response")
    }
}
#[tokio::test]
async fn admitted_stream_delivers_external_user_changes_replays_and_fences_detach() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    let response = f
        .authorize(f.client.get(path(&f.base, &client)))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    let mut stream = Frames {
        response,
        buffer: String::new(),
    };
    let (kind, ready) = stream.next().await.unwrap();
    assert_eq!(kind, "ready");
    std::fs::write(
        f.roots[0].path().join("visible.md"),
        "private content never sent",
    )
    .unwrap();
    let frame = loop {
        let (_, frame) = stream.next().await.unwrap();
        if serde_json::to_string(&frame)
            .unwrap()
            .contains("visible.md")
        {
            break frame;
        }
    };
    let encoded = serde_json::to_string(&frame).unwrap();
    assert!(
        !encoded.contains("private content")
            && !encoded.contains(&f.roots[0].path().to_string_lossy().to_string())
    );
    let response = f
        .authorize(f.client.get(format!(
            "{}&cursor={}",
            path(&f.base, &client),
            ready.revision
        )))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let mut replay = Frames {
        response,
        buffer: String::new(),
    };
    assert!(!replay.next().await.unwrap().1.reset);
    assert!(
        serde_json::to_string(&replay.next().await.unwrap().1)
            .unwrap()
            .contains("visible.md")
    );
    f.detach(&client).await;
    std::fs::write(f.roots[0].path().join("after-detach.md"), "private").unwrap();
    while let Some((_, frame)) = stream.next().await {
        assert!(
            !serde_json::to_string(&frame)
                .unwrap()
                .contains("after-detach")
        );
    }
    while let Some((_, frame)) = replay.next().await {
        assert!(
            !serde_json::to_string(&frame)
                .unwrap()
                .contains("after-detach")
        );
    }
}
#[tokio::test]
async fn stream_rejects_auth_origin_foreign_scope_and_invalid_cursor() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    assert_eq!(
        f.client
            .get(path(&f.base, &client))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        f.authorize(f.client.get(path(&f.base, &client)))
            .header("origin", "https://foreign.invalid")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let mut foreign = client.clone();
    foreign.vault_id = f.ids[1];
    assert_eq!(
        f.authorize(f.client.get(path(&f.base, &foreign)))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    foreign = client.clone();
    foreign.generation = uuid::Uuid::new_v4();
    assert_eq!(
        f.authorize(f.client.get(path(&f.base, &foreign)))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.authorize(f.client.get(format!(
            "{}&cursor=9007199254740992",
            path(&f.base, &client)
        )))
        .send()
        .await
        .unwrap()
        .status(),
        400
    );
    f.detach(&client).await;
}
#[tokio::test]
async fn ordinary_file_named_events_retains_its_existing_byte_route() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    std::fs::write(f.roots[0].path().join("events"), "ordinary user file").unwrap();
    let response = f
        .authorize(f.client.get(format!(
            "{}/api/v3/clients/{}/files/events",
            f.base, client.client_id
        )))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(response.text().await.unwrap(), "ordinary user file");
    f.detach(&client).await;
}
#[tokio::test]
async fn revoked_browser_session_closes_an_idle_file_stream() {
    let mut f = browser::BrowserFixture::start(true, None).await;
    let (cookie, info, _) = f.login().await;
    let client = f.attach(&cookie, &info).await;
    let response = f
        .authorize(f.client.get(path(&f.base, &client)), &cookie, &info, false)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let mut stream = Frames {
        response,
        buffer: String::new(),
    };
    assert_eq!(stream.next().await.unwrap().0, "ready");
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
    assert!(stream.next().await.is_none());
    f.finish().await;
}
