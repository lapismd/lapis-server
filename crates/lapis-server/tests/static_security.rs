#![cfg(unix)]
#[path = "support/static_delivery/mod.rs"]
mod support;
use std::{os::unix::fs::symlink, time::Duration};
use support::Fixture;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::timeout,
};

async fn raw(base: &str, path: &str) -> String {
    let mut socket = tokio::net::TcpStream::connect(base.strip_prefix("http://").unwrap())
        .await
        .unwrap();
    socket
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .unwrap();
    let mut bytes = Vec::new();
    timeout(Duration::from_secs(3), socket.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[tokio::test]
async fn malformed_hidden_traversal_symlink_and_nonregular_paths_never_publish_bytes() {
    let fixture = Fixture::new();
    std::fs::write(fixture.vault.join("secret.txt"), "private-vault-secret").unwrap();
    std::fs::write(fixture.assets.join(".env"), "private-hidden-secret").unwrap();
    symlink(
        fixture.vault.join("secret.txt"),
        fixture.assets.join("leak"),
    )
    .unwrap();
    symlink(&fixture.vault, fixture.assets.join("alias")).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(fixture.assets.join("pipe"))
            .status()
            .unwrap()
            .success()
    );
    let (_child, base) = fixture.start().await;
    assert_eq!(
        reqwest::get(format!("{base}/")).await.unwrap().status(),
        200
    );
    for path in [
        "/%2e%2e/server.json",
        "/%252e%252e/server.json",
        "/.env",
        "/leak",
        "/alias/secret.txt",
        "/pipe",
        "/%00",
        "/%FF",
        "/%ZZ",
        "/a%5cb",
        "/api%2ftrpc/system",
        "/__lapis%2fplugins/missing",
        "//",
    ] {
        let response = raw(&base, path).await;
        assert!(!response.starts_with("HTTP/1.1 200"), "{path}: {response}");
        for text in [
            "private-vault-secret",
            "private-hidden-secret",
            "Lapis fixture",
            fixture.root.path().to_str().unwrap(),
        ] {
            assert!(!response.contains(text), "{path}");
        }
    }
}

#[tokio::test]
async fn static_root_replacement_fails_closed_and_ordinary_assets_revalidate() {
    let fixture = Fixture::new();
    let (_child, base) = fixture.start().await;
    let client = reqwest::Client::new();
    let response = client.get(format!("{base}/app.css")).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let etag = response.headers()["etag"].clone();
    std::fs::write(fixture.assets.join("app.css"), "body { color: blue }").unwrap();
    let changed = client
        .get(format!("{base}/app.css"))
        .header("if-none-match", &etag)
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), 200);
    assert_ne!(changed.headers()["etag"], etag);
    assert_eq!(changed.text().await.unwrap(), "body { color: blue }");
    std::fs::rename(&fixture.assets, fixture.root.path().join("old-static")).unwrap();
    std::fs::create_dir(&fixture.assets).unwrap();
    std::fs::write(fixture.assets.join("index.html"), "replacement-index").unwrap();
    let denied = client.get(format!("{base}/")).send().await.unwrap();
    assert!(!denied.status().is_success());
    let text = denied.text().await.unwrap();
    assert!(!text.contains("replacement-index"));
    assert!(!text.contains(fixture.root.path().to_str().unwrap()));
}
