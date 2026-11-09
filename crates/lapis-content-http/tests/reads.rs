use lapis_app_core::services::{
    Runtime,
    content::{ContentGrants, ContentVault},
};
use lapis_app_protocol::content::*;
use lapis_content_http::{API_HEADER, READ_ROUTES, REQUEST_ID_HEADER, router};
use reqwest::{Client, RequestBuilder, StatusCode};
use std::{fs, sync::Arc, time::Duration};
use uuid::Uuid;

struct Fixture {
    runtime: Runtime,
    root: tempfile::TempDir,
    vault: Uuid,
    grants: Arc<ContentGrants>,
    grant: Uuid,
    token: String,
    base: String,
    client: Client,
    server: tokio::task::JoinHandle<()>,
}
impl Fixture {
    async fn start() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a note.md"), "# Saved 🪨\r\n").unwrap();
        fs::write(root.path().join("b.md"), "second").unwrap();
        let runtime = Runtime::default();
        let vault = Uuid::new_v4();
        let admitted = runtime.admit(vault, root.path()).unwrap();
        let grants = Arc::new(ContentGrants::default());
        let issued = grants
            .issue_read_only(
                vec![ContentVault {
                    name: "Personal".into(),
                    client: admitted.attach().unwrap(),
                }],
                Duration::from_secs(60),
            )
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = router(grants.clone(), &base).unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            runtime,
            root,
            vault,
            grants,
            grant: issued.grant_id,
            token: issued.token.expose_secret().into(),
            base,
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            server,
        }
    }
    fn get(&self, path: &str) -> RequestBuilder {
        self.authorize(
            self.client
                .get(format!("{}/api/content/v1{path}", self.base)),
        )
    }
    fn authorize(&self, builder: RequestBuilder) -> RequestBuilder {
        builder.bearer_auth(&self.token).header(API_HEADER, "1")
    }
    fn note_path(&self) -> String {
        format!("/vaults/{}/note", self.vault)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
        self.grants.shutdown().unwrap();
    }
}
async fn value(response: reqwest::Response) -> ContentValue {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()[API_HEADER], "1");
    let header_id = response.headers()[REQUEST_ID_HEADER]
        .to_str()
        .unwrap()
        .to_owned();
    let reply: ContentReply = response.json().await.unwrap();
    assert_eq!(reply.request_id.to_string(), header_id);
    match reply.outcome {
        ContentOutcome::Ok { value } => value,
        ContentOutcome::Error { error } => panic!("{error:?}"),
    }
}
async fn error(response: reqwest::Response, status: StatusCode, code: ContentErrorCode) {
    assert_eq!(response.status(), status);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(
        !response
            .headers()
            .contains_key("access-control-allow-origin")
    );
    let reply: ContentReply = response.json().await.unwrap();
    let ContentOutcome::Error { error } = reply.outcome else {
        panic!()
    };
    assert_eq!(error.code, code);
    assert_eq!(error.effect, ContentEffectStatus::NotStarted);
}

#[tokio::test]
async fn real_http_reads_match_native_content_versions_and_the_operation_catalog() {
    let f = Fixture::start().await;
    let ContentValue::Capabilities { capabilities } =
        value(f.get("/capabilities").send().await.unwrap()).await
    else {
        panic!()
    };
    assert_eq!(capabilities.operations.len(), READ_ROUTES.len());
    for operation in &capabilities.operations {
        assert!(READ_ROUTES.iter().any(|(id, _)| *id == operation.operation));
        assert_eq!(operation.effect, ContentEffect::ReadOnly);
    }
    let ContentValue::Vaults { items, .. } = value(f.get("/vaults").send().await.unwrap()).await
    else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].vault_id, f.vault);
    let id = Uuid::new_v4();
    let response = f
        .get(&f.note_path())
        .query(&[("path", "a note.md")])
        .header(REQUEST_ID_HEADER, id.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[REQUEST_ID_HEADER], id.to_string());
    assert_eq!(
        response.headers()["content-type"],
        "text/plain; charset=utf-8"
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    let etag = response.headers()["etag"].to_str().unwrap().to_owned();
    let native = f
        .runtime
        .admit(f.vault, f.root.path())
        .unwrap()
        .attach()
        .unwrap()
        .read_text("a note.md")
        .unwrap();
    assert_eq!(etag, format!("\"{}\"", native.version));
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        native.text.as_bytes()
    );
}

#[tokio::test]
async fn pagination_retains_core_scope_and_snapshot_conflicts() {
    let f = Fixture::start().await;
    let path = format!("/vaults/{}/notes", f.vault);
    let ContentValue::Notes {
        items,
        next_cursor: Some(cursor),
    } = value(f.get(&path).query(&[("limit", "1")]).send().await.unwrap()).await
    else {
        panic!()
    };
    assert_eq!(items[0].path, "a note.md");
    let ContentValue::Notes {
        items,
        next_cursor: None,
    } = value(
        f.get(&path)
            .query(&[("limit", "1"), ("cursor", &cursor)])
            .send()
            .await
            .unwrap(),
    )
    .await
    else {
        panic!()
    };
    assert_eq!(items[0].path, "b.md");
    fs::write(f.root.path().join("c.md"), "new").unwrap();
    error(
        f.get(&path)
            .query(&[("limit", "1"), ("cursor", &cursor)])
            .send()
            .await
            .unwrap(),
        StatusCode::CONFLICT,
        ContentErrorCode::Conflict,
    )
    .await;
    error(
        f.get(&format!("/vaults/{}/notes", Uuid::new_v4()))
            .send()
            .await
            .unwrap(),
        StatusCode::FORBIDDEN,
        ContentErrorCode::PermissionDenied,
    )
    .await;
}

#[path = "reads/security.rs"]
mod security;
