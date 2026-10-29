use super::Fixture;
use lapis_app_protocol::{ManagedVault, Outcome, VaultCommand, VaultValue};
use std::io::{Cursor, Read};

const NOTES: &[u8] = include_bytes!("../../../lapis-app-core/tests/fixtures/archives/notes.zip");

#[tokio::test]
async fn incomplete_or_oversized_uploads_never_enter_the_managed_catalogue() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut f = Fixture::start().await;
    for (length, body, expected) in [
        (20, b"part".as_slice(), "HTTP/1.1 400"),
        (536870913, b"".as_slice(), "HTTP/1.1 413"),
    ] {
        let mut socket = tokio::net::TcpStream::connect(f.base.strip_prefix("http://").unwrap())
            .await
            .unwrap();
        let headers = format!(
            "POST /api/v3/vaults/import?name=Incomplete HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\nx-lapis-protocol: 3\r\nConnection: close\r\nContent-Length: {length}\r\n\r\n",
            super::TOKEN
        );
        socket.write_all(headers.as_bytes()).await.unwrap();
        socket.write_all(body).await.unwrap();
        socket.shutdown().await.unwrap();
        let mut reply = Vec::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            socket.read_to_end(&mut reply),
        )
        .await
        .unwrap()
        .unwrap();
        let reply = String::from_utf8_lossy(&reply);
        assert!(
            reply.starts_with(expected) || (length == 20 && reply.is_empty()),
            "{reply}"
        );
        assert_eq!(f.list().await.len(), 1);
        assert_eq!(
            std::fs::read_dir(f.profile.join("vaults")).unwrap().count(),
            0
        );
    }
    f.finish().await;
}

#[tokio::test]
async fn transfer_import_publishes_complete_native_content_and_retains_it_after_restart() {
    let f = Fixture::start().await;
    let response = f
        .post("/api/v3/vaults/import?name=Imported")
        .header("content-type", "application/zip")
        .body(NOTES)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        201,
        "the retained import control can publish a Rust-owned vault"
    );
    let imported: ManagedVault = response.json().await.unwrap();
    let id = imported.vault_id;
    assert_eq!(imported.name, "Imported");
    let content = f.profile.join(format!("vaults/{id}/content"));
    assert_eq!(
        std::fs::read(content.join("image.bin")).unwrap(),
        vec![0, 128, 255]
    );
    assert_eq!(
        std::fs::read_to_string(content.join("notes/café.md")).unwrap(),
        "# Imported café\n"
    );
    assert_eq!(
        std::fs::read_to_string(content.join(".lapis/app.json")).unwrap(),
        "{\"appearance\":\"dark\"}"
    );
    let mut f = f.restart().await;
    let client = f.attach(id).await;
    assert!(
        matches!(f.execute(&client, VaultCommand::ReadText { path: "notes/café.md".into() }).await.outcome, Outcome::Ok { value: VaultValue::Text { snapshot } } if snapshot.text == "# Imported café\n")
    );
    f.finish().await;
}

#[tokio::test]
async fn invalid_archives_do_not_publish_or_leave_partial_content() {
    let mut f = Fixture::start().await;
    for bytes in [
        b"broken archive".as_slice(),
        include_bytes!("../../../lapis-app-core/tests/fixtures/archives/traversal.zip").as_slice(),
    ] {
        let response = f
            .post("/api/v3/vaults/import?name=Rejected")
            .body(bytes)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(f.list().await.len(), 1);
        assert_eq!(
            std::fs::read_dir(f.profile.join("vaults")).unwrap().count(),
            0
        );
    }
    f.finish().await;
}

#[tokio::test]
async fn export_is_an_independent_bounded_snapshot_and_preserves_content_on_roundtrip() {
    let mut f = Fixture::start().await;
    let vault: ManagedVault = f
        .post("/api/v3/vaults/import?name=Imported")
        .body(NOTES)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let url = format!("{}/api/v3/vaults/{}/export", f.base, vault.vault_id);
    let response = reqwest::Client::new()
        .get(&url)
        .bearer_auth(super::TOKEN)
        .header("x-lapis-protocol", 3)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "application/zip");
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(
        response.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("attachment; filename=\"lapis-")
    );
    let length: usize = response.headers()["content-length"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    // Archive can drain the vault while the HTTP response retains its snapshot.
    f.action(vault.vault_id, "archive").await;
    let bytes = response.bytes().await.unwrap();
    assert_eq!(bytes.len(), length);
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut binary = Vec::new();
    archive
        .by_name("image.bin")
        .unwrap()
        .read_to_end(&mut binary)
        .unwrap();
    assert_eq!(binary, vec![0, 128, 255]);
    let mut state = String::new();
    archive
        .by_name(".lapis/app.json")
        .unwrap()
        .read_to_string(&mut state)
        .unwrap();
    assert_eq!(state, "{\"appearance\":\"dark\"}");
    assert_eq!(
        reqwest::Client::new()
            .get(url)
            .bearer_auth(super::TOKEN)
            .header("x-lapis-protocol", 3)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    f.finish().await;
}

#[tokio::test]
async fn cookie_export_downloads_preserve_origin_version_and_mutation_guards() {
    let mut f = Fixture::start().await;
    let login = reqwest::Client::new()
        .post(format!("{}/api/v3/auth/login", f.base))
        .header("origin", super::ORIGIN)
        .header("x-lapis-protocol", 3)
        .json(&serde_json::json!({"username":"owner","password":crate::credentials::PASSWORD}))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), 200);
    let cookie = login.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let url = format!("{}/api/v3/vaults/{}/export", f.base, f.configured);
    let client = reqwest::Client::new();
    for method in [reqwest::Method::GET, reqwest::Method::HEAD] {
        let response = client
            .request(method.clone(), &url)
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert!(
            response.headers()["content-length"]
                .to_str()
                .unwrap()
                .parse::<usize>()
                .unwrap()
                >= 22
        );
        if method == reqwest::Method::HEAD {
            assert!(response.bytes().await.unwrap().is_empty());
        }
    }
    for (request, status) in [
        (client.get(&url), 401),
        (client.get(&url).bearer_auth(super::TOKEN), 409),
        (
            client
                .get(&url)
                .header("cookie", &cookie)
                .header("x-lapis-protocol", 2),
            409,
        ),
        (
            client
                .get(&url)
                .header("cookie", &cookie)
                .header("origin", "https://evil.example"),
            403,
        ),
        (
            client
                .post(format!("{}/api/v3/vaults/import?name=Blocked", f.base))
                .header("cookie", &cookie)
                .header("x-lapis-protocol", 3)
                .header("origin", super::ORIGIN)
                .body(NOTES),
            403,
        ),
    ] {
        assert_eq!(request.send().await.unwrap().status(), status);
    }
    assert_eq!(f.list().await.len(), 1);
    f.finish().await;
}
