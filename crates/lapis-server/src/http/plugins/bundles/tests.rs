use super::*;
use axum::{Router, routing::get};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[test]
fn release_sources_and_redirects_keep_the_existing_host_allowlist() {
    for (url, allowed) in [
        ("https://registry.lapis.md/download/wordcount/0.1.2", true),
        ("https://registry.lapis.md/admin/wordcount/0.1.2", false),
        (
            "https://registry.lapis.md/download/wordcount/0.1.2?token=private",
            false,
        ),
        (
            "https://github.com/lapismd/lapis-plugins/releases/download/v1/file.lapis-plugin",
            true,
        ),
        (
            "https://github.com/lapis-notes/releases/releases/download/v1/file.lapis-plugin",
            true,
        ),
        (
            "http://github.com/lapismd/lapis-plugins/releases/download/v1/file.lapis-plugin",
            false,
        ),
        (
            "https://github.com/other/repo/releases/download/v1/file.lapis-plugin",
            false,
        ),
        (
            "https://user:secret@github.com/lapismd/lapis-plugins/releases/download/v1/file.lapis-plugin",
            false,
        ),
        (
            "https://github.com/lapismd/lapis-plugins/releases/download/v1/file.lapis-plugin?token=private",
            false,
        ),
        (
            "https://github.com/lapismd/lapis-plugins/releases/download/v1/file.lapis-plugin#hash",
            false,
        ),
        (
            "https://github.com.evil.example/lapismd/lapis-plugins/releases/download/v1/file.lapis-plugin",
            false,
        ),
    ] {
        assert_eq!(official(url), allowed, "{url}");
    }
    for (url, allowed) in [
        (
            "https://release-assets.githubusercontent.com/asset?public-signature=value",
            true,
        ),
        ("https://objects.githubusercontent.com/asset", true),
        ("http://release-assets.githubusercontent.com/asset", false),
        ("https://evil.example/asset", false),
        ("https://private@github.com/asset", false),
    ] {
        assert_eq!(redirect_allowed(&url.parse().unwrap()), allowed);
    }
}

async fn upstream(declared: Option<u64>) -> (tokio::task::JoinHandle<()>, reqwest::Response) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/bundle",
        get(move || async move {
            let stream = futures_util::stream::iter([Ok::<_, std::io::Error>(
                axum::body::Bytes::from_static(b"hello"),
            )]);
            let mut builder = Response::builder();
            if let Some(size) = declared {
                builder = builder.header("content-length", size);
            }
            builder.body(Body::from_stream(stream)).unwrap()
        }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let response = reqwest::Client::new()
        .get(format!("http://{address}/bundle"))
        .send()
        .await
        .unwrap();
    (server, response)
}
#[tokio::test]
async fn public_bundle_bytes_are_bounded_before_a_complete_snapshot_is_returned() {
    for declared in [None, Some(5)] {
        let (server, response) = upstream(declared).await;
        let failure = bounded(response, 4).await.err().unwrap();
        assert_eq!(failure.0, StatusCode::PAYLOAD_TOO_LARGE);
        server.abort();
        let _ = server.await;
    }
    let (server, response) = upstream(None).await;
    let mut snapshot = bounded(response, 5).await.ok().unwrap();
    assert_eq!(snapshot.length, 5);
    let mut bytes = Vec::new();
    snapshot.file.read_to_end(&mut bytes).await.unwrap();
    assert_eq!(bytes, b"hello");
    server.abort();
    let _ = server.await;
}
#[tokio::test]
async fn incomplete_upstream_bytes_never_become_a_download_snapshot() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 2048];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\npart")
            .await
            .unwrap();
        socket.shutdown().await.unwrap();
    });
    let response = reqwest::Client::new()
        .get(format!("http://{address}/bundle"))
        .send()
        .await
        .unwrap();
    let failure = bounded(response, 10).await.err().unwrap();
    assert_eq!(failure.0, StatusCode::NOT_FOUND);
    server.await.unwrap();
}
