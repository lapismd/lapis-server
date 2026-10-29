mod support;
use lapis_app_protocol::{
    VaultCommand,
    files::{FileCommand, FileOptions},
};
use serde_json::json;
use sha2::{Digest, Sha256};

#[tokio::test]
async fn bundle_download_admission_rejects_unapproved_sources_before_network_io() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    let url = format!(
        "{}/api/v3/clients/{}/plugin-bundles/download",
        f.base, client.client_id
    );
    for source in [
        "http://127.0.0.1/private",
        "https://evil.example/bundle.lapis-plugin",
        "https://github.com/other/repo/releases/download/v1/file.lapis-plugin",
    ] {
        let response = f
            .authorize(f.client.get(&url))
            .query(&[("source", source)])
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
    }
    f.host.shutdown().unwrap();
}
use support::Fixture;

#[tokio::test]
async fn installed_modules_use_the_admitted_native_owner_and_fingerprint() {
    let f = Fixture::start().await;
    let root = f.roots[0].path();
    std::fs::create_dir_all(root.join(".lapis/plugins/portable")).unwrap();
    let bytes = b"export const marker = 1;";
    let hash = format!("{:x}", Sha256::digest(bytes));
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(format!("main.mjs:{}:{hash}", bytes.len()))
    );
    std::fs::write(root.join(".lapis/plugins/portable/main.mjs"), bytes).unwrap();
    std::fs::write(root.join(".lapis/installed-plugins.json"), json!({"schemaVersion":1,"plugins":{"portable":{"pluginId":"portable","installedVersion":"1.0.0","files":[{"path":"main.mjs","size":bytes.len(),"sha256":hash}]}}}).to_string()).unwrap();
    let client = f.attach(f.ids[0]).await;
    let response = f
        .authorize(f.client.get(format!(
            "{}/__lapis/plugins/{}/portable/1.0.0/{fingerprint}/main.mjs",
            f.base, client.client_id
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "the retained runtime can fetch a verified installed module"
    );
    assert_eq!(
        response.headers()["content-type"],
        "text/javascript; charset=utf-8"
    );
    assert_eq!(response.bytes().await.unwrap().as_ref(), bytes);
    let url = format!(
        "{}/__lapis/plugins/{}/portable/1.0.0/{fingerprint}/main.mjs",
        f.base, client.client_id
    );
    let response = f.authorize(f.client.head(&url)).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()["content-length"],
        bytes.len().to_string()
    );
    assert!(response.bytes().await.unwrap().is_empty());
    let response = f
        .authorize(f.client.get(&url))
        .header("range", "bytes=0-5")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 206);
    assert_eq!(response.bytes().await.unwrap().as_ref(), &bytes[..6]);
    assert_eq!(
        f.authorize(f.client.get(&url))
            .header("if-none-match", format!("\"{hash}\""))
            .send()
            .await
            .unwrap()
            .status(),
        304
    );
    f.execute(
        &client,
        VaultCommand::Files {
            operation: FileCommand::WriteText {
                path: ".lapis/plugins/portable/main.mjs".into(),
                data: "export const marker = 2;".into(),
                options: FileOptions::default(),
            },
        },
    )
    .await;
    assert_eq!(
        f.authorize(f.client.get(&url))
            .header("if-none-match", format!("\"{hash}\""))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    f.detach(&client).await;
    assert_eq!(
        f.authorize(f.client.get(&url))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    f.host.shutdown().unwrap();
}

#[tokio::test]
async fn installed_assets_keep_http_authority_and_private_profile_boundaries() {
    let f = Fixture::with_database().await;
    let attached = f.attach(f.ids[0]).await;
    let url = format!(
        "{}/__lapis/plugins/{}/portable/1.0.0/{}/main.mjs",
        f.base,
        attached.client_id,
        "a".repeat(64)
    );
    for (request, status) in [
        (f.client.get(&url), 401),
        (f.client.get(&url).bearer_auth(support::TOKEN), 409),
        (
            f.authorize(f.client.get(&url))
                .header("origin", "https://foreign.invalid"),
            403,
        ),
        (f.authorize(f.client.get(&url)), 404),
    ] {
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), status);
        assert!(
            !response
                .text()
                .await
                .unwrap()
                .contains(f.profile.as_ref().unwrap().path().to_str().unwrap())
        );
    }
    let host = f.host.clone();
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}
