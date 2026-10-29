use super::{Fixture, url};
use lapis_app_protocol::{
    Outcome, VaultCommand, VaultValue,
    files::{FileCommand, FileValue},
};
use sha2::{Digest, Sha256};
use std::{io, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn chunked_binary_upload_keeps_exact_bytes_mtime_ranges_and_cache_validators() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    let destination = url(&f, &client, "caf%C3%A9.bin");
    let chunks: Vec<Result<Vec<u8>, io::Error>> = vec![
        Ok(vec![0; 2 * 1024 * 1024]),
        Ok(vec![255; 2 * 1024 * 1024]),
        Ok(vec![42; 2 * 1024 * 1024]),
    ];
    let response = f
        .authorize(f.client.put(&destination))
        .header("if-none-match", "*")
        .header("x-lapis-mtime", "1700000000000")
        .body(reqwest::Body::wrap_stream(futures_util::stream::iter(
            chunks,
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    let etag = response.headers()["etag"].to_str().unwrap().to_owned();
    let stored = std::fs::read(f.roots[0].path().join("café.bin")).unwrap();
    assert_eq!(stored.len(), 6 * 1024 * 1024);
    assert_eq!(etag, format!("\"{:x}\"", Sha256::digest(&stored)));
    assert!(stored[..2 * 1024 * 1024].iter().all(|byte| *byte == 0));
    assert!(
        stored[2 * 1024 * 1024..4 * 1024 * 1024]
            .iter()
            .all(|byte| *byte == 255)
    );
    assert!(stored[4 * 1024 * 1024..].iter().all(|byte| *byte == 42));
    let stat = f
        .execute(
            &client,
            VaultCommand::Files {
                operation: FileCommand::Stat {
                    path: "café.bin".into(),
                },
            },
        )
        .await;
    assert!(
        matches!(stat.outcome, Outcome::Ok { value: VaultValue::Files { value: FileValue::Stat { stat: Some(stat) } } } if stat.mtime == 1700000000000.0)
    );
    let head = f
        .authorize(f.client.head(&destination))
        .send()
        .await
        .unwrap();
    assert_eq!(head.status(), 200);
    assert_eq!(head.headers()["content-length"], stored.len().to_string());
    assert!(head.bytes().await.unwrap().is_empty());
    let range = f
        .authorize(f.client.get(&destination))
        .header("range", "bytes=2097150-2097154")
        .header("if-range", &etag)
        .send()
        .await
        .unwrap();
    assert_eq!(range.status(), 206);
    assert_eq!(
        range.headers()["content-range"],
        "bytes 2097150-2097154/6291456"
    );
    assert_eq!(range.bytes().await.unwrap().as_ref(), [0, 0, 255, 255, 255]);
    let suffix = f
        .authorize(f.client.get(&destination))
        .header("range", "bytes=-4")
        .send()
        .await
        .unwrap();
    assert_eq!(suffix.status(), 206);
    assert_eq!(suffix.bytes().await.unwrap().as_ref(), [42; 4]);
    let ignored = f
        .authorize(f.client.get(&destination))
        .header("range", "bytes=1-2")
        .header("if-range", "\"old\"")
        .send()
        .await
        .unwrap();
    assert_eq!(ignored.status(), 200);
    assert_eq!(ignored.content_length(), Some(stored.len() as u64));
    drop(ignored);
    let cached = f
        .authorize(f.client.get(&destination))
        .header("if-none-match", format!("W/{etag}"))
        .send()
        .await
        .unwrap();
    assert_eq!(cached.status(), 304);
    assert!(cached.bytes().await.unwrap().is_empty());
    for range in [
        "bytes=999999999-",
        "bytes=-0",
        "bytes=3-1",
        "bytes=1-2,4-5",
        "bytes=+1-2",
    ] {
        let response = f
            .authorize(f.client.get(&destination))
            .header("range", range)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 416, "{range}");
        assert_eq!(response.headers()["content-range"], "bytes */6291456");
    }
    f.detach(&client).await;
    f.host.shutdown().unwrap();
}

#[tokio::test]
async fn raw_put_preserves_conditional_writes_and_detached_dirty_editor_guards() {
    let f = Fixture::start().await;
    let a = f.attach(f.ids[0]).await;
    let b = f.attach(f.ids[0]).await;
    let first = f
        .authorize(f.client.put(url(&f, &a, "note.md")))
        .header("if-none-match", "*")
        .body("original")
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 204);
    let old = first.headers()["etag"].clone();
    let editor = match f
        .execute(
            &a,
            VaultCommand::OpenEditor {
                path: "note.md".into(),
            },
        )
        .await
        .outcome
    {
        Outcome::Ok {
            value: VaultValue::Editor { editor_id },
        } => editor_id,
        other => panic!("{other:?}"),
    };
    assert!(matches!(
        f.execute(
            &a,
            VaultCommand::MarkDirty {
                editor_id: editor,
                revision: 1
            }
        )
        .await
        .outcome,
        Outcome::Ok { .. }
    ));
    f.detach(&a).await;
    let path = url(&f, &b, "note.md");
    let blocked = f
        .authorize(f.client.put(&path))
        .header("if-match", &old)
        .body("unsafe")
        .send()
        .await
        .unwrap();
    assert_eq!(blocked.status(), 409);
    assert!(matches!(
        f.execute(&b, VaultCommand::RecoverEditor { editor_id: editor })
            .await
            .outcome,
        Outcome::Ok { .. }
    ));
    assert!(matches!(
        f.execute(&b, VaultCommand::DiscardEditor { editor_id: editor })
            .await
            .outcome,
        Outcome::Ok { .. }
    ));
    let saved = f
        .authorize(f.client.put(&path))
        .header("if-match", &old)
        .body("persisted")
        .send()
        .await
        .unwrap();
    assert_eq!(saved.status(), 204);
    let stale = f
        .authorize(f.client.put(&path))
        .header("if-match", &old)
        .body("unsafe")
        .send()
        .await
        .unwrap();
    assert_eq!(stale.status(), 412);
    let collision = f
        .authorize(f.client.put(&path))
        .header("if-none-match", "*")
        .body("unsafe")
        .send()
        .await
        .unwrap();
    assert_eq!(collision.status(), 412);
    let invalid = f
        .authorize(f.client.put(&path))
        .header("if-match", "W/\"invalid\"")
        .body("unsafe")
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 400);
    assert_eq!(
        std::fs::read_to_string(f.roots[0].path().join("note.md")).unwrap(),
        "persisted"
    );
    f.detach(&b).await;
    assert_eq!(
        f.authorize(f.client.get(&path))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.authorize(f.client.put(&path))
            .body("unsafe")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    f.host.shutdown().unwrap();
}

async fn raw(f: &Fixture, headers: String, partial: &[u8]) -> String {
    let mut socket = tokio::net::TcpStream::connect(f.base.strip_prefix("http://").unwrap())
        .await
        .unwrap();
    socket.write_all(headers.as_bytes()).await.unwrap();
    socket.write_all(partial).await.unwrap();
    socket.shutdown().await.unwrap();
    let mut reply = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), socket.read_to_end(&mut reply))
        .await
        .unwrap()
        .unwrap();
    String::from_utf8_lossy(&reply).into_owned()
}
#[tokio::test]
async fn interrupted_and_oversized_uploads_cannot_publish_partial_bytes() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    std::fs::write(f.roots[0].path().join("note.md"), "original").unwrap();
    let prefix = format!(
        "PUT /api/v3/clients/{}/files/note.md HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\nx-lapis-protocol: 3\r\nConnection: close\r\n",
        client.client_id,
        super::super::support::TOKEN
    );
    let reply = raw(&f, format!("{prefix}Content-Length: 8\r\n\r\n"), b"part").await;
    assert!(
        reply.is_empty() || reply.starts_with("HTTP/1.1 400"),
        "{reply}"
    );
    let reply = raw(&f, format!("{prefix}Content-Length: 67108865\r\n\r\n"), b"").await;
    assert!(reply.starts_with("HTTP/1.1 413"), "{reply}");
    let chunks = (0..33).map(|_| Ok::<_, io::Error>(vec![42; 2 * 1024 * 1024]));
    let response = f
        .authorize(f.client.put(url(&f, &client, "note.md")))
        .body(reqwest::Body::wrap_stream(futures_util::stream::iter(
            chunks,
        )))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
    assert_eq!(response.headers()["connection"], "close");
    assert_eq!(
        std::fs::read_to_string(f.roots[0].path().join("note.md")).unwrap(),
        "original"
    );
    f.detach(&client).await;
    f.host.shutdown().unwrap();
}
