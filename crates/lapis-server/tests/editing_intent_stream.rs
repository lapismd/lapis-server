#[path = "browser_auth/support.rs"]
#[allow(dead_code)]
mod browser;
#[allow(dead_code)]
mod credentials;
#[allow(dead_code)]
mod support;
use lapis_app_protocol::{ClientDescriptor, Outcome, VaultCommand};
use serde_json::{Value, json};
use std::time::Duration;
use support::Fixture;
use uuid::Uuid;

fn path(base: &str, client: &ClientDescriptor) -> String {
    format!(
        "{base}/api/v3/clients/{}/editing-intent-events?vault_id={}&generation={}",
        client.client_id, client.vault_id, client.generation
    )
}
fn intent(command: Value) -> VaultCommand {
    VaultCommand::EditingIntents {
        command: serde_json::from_value(command).unwrap(),
    }
}
struct Frames {
    response: reqwest::Response,
    buffer: String,
}
impl Frames {
    async fn next(&mut self) -> Option<(String, Value)> {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                while let Some(end) = self.buffer.find("\n\n") {
                    let frame = self.buffer[..end].to_owned();
                    self.buffer.drain(..end + 2);
                    let kind = frame.lines().find_map(|line| line.strip_prefix("event: "));
                    let data = frame.lines().find_map(|line| line.strip_prefix("data: "));
                    if let (Some(kind), Some(data)) = (kind, data) {
                        let value: Value = serde_json::from_str(data).unwrap();
                        assert_eq!(
                            value.as_object().unwrap().keys().collect::<Vec<_>>(),
                            ["revision"]
                        );
                        return Some((kind.to_owned(), value));
                    }
                }
                let bytes = self.response.chunk().await.unwrap()?;
                self.buffer.push_str(std::str::from_utf8(&bytes).unwrap());
                assert!(self.buffer.len() < 1024);
            }
        })
        .await
        .expect("bounded editing-intent observation")
    }
}

#[tokio::test]
async fn admitted_editing_intent_observation_registers_without_claim_or_content_bytes() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    let response = f
        .authorize(f.client.get(path(&f.base, &client)))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "editing-intent invalidation stream is required"
    );
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    let mut stream = Frames {
        response,
        buffer: String::new(),
    };
    assert_eq!(
        stream.next().await.unwrap(),
        ("ready".into(), json!({"revision": 0}))
    );
    let writer = f.attach(f.ids[0]).await;
    let editor = Uuid::new_v4();
    std::fs::write(f.roots[0].path().join("private.md"), "private saved bytes").unwrap();
    for (who, command, revision) in [
        (
            &writer,
            json!({"kind":"acquire", "editor_id":editor,"path":"private.md"}),
            1,
        ),
        (
            &writer,
            json!({"kind":"changed", "editor_id":editor,"generation":1,"revision":3}),
            2,
        ),
    ] {
        assert!(matches!(
            f.execute(who, intent(command)).await.outcome,
            Outcome::Ok { .. }
        ));
        assert_eq!(
            stream.next().await.unwrap(),
            ("invalidate".into(), json!({"revision":revision}))
        );
    }
    f.detach(&writer).await;
    assert_eq!(stream.next().await.unwrap().1, json!({"revision":3}));
    for (command, revision) in [
        (
            json!({"kind":"recover", "editor_id":editor,"generation":1,"revision":3}),
            4,
        ),
        (
            json!({"kind":"discard", "editor_id":editor,"generation":2,"revision":3}),
            5,
        ),
    ] {
        assert!(matches!(
            f.execute(&client, intent(command)).await.outcome,
            Outcome::Ok { .. }
        ));
        assert_eq!(stream.next().await.unwrap().1, json!({"revision":revision}));
    }
    assert_eq!(
        std::fs::read_to_string(f.roots[0].path().join("private.md")).unwrap(),
        "private saved bytes"
    );
    f.detach(&client).await;
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn editing_observation_rejects_auth_origin_foreign_scope_and_unknown_fields() {
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
    for descriptor in [
        ClientDescriptor {
            vault_id: f.ids[1],
            ..client.clone()
        },
        ClientDescriptor {
            generation: Uuid::new_v4(),
            ..client.clone()
        },
    ] {
        assert_eq!(
            f.authorize(f.client.get(path(&f.base, &descriptor)))
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
    assert_eq!(
        f.authorize(
            f.client
                .get(format!("{}&root=/tmp", path(&f.base, &client)))
        )
        .send()
        .await
        .unwrap()
        .status(),
        400
    );
    f.detach(&client).await;
    assert_eq!(
        f.authorize(f.client.get(path(&f.base, &client)))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
}

#[tokio::test]
async fn revoked_browser_session_closes_an_idle_editing_observation() {
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
