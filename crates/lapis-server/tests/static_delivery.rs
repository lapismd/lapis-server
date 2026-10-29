#![cfg(unix)]
#[path = "support/static_delivery/mod.rs"]
mod support;
use reqwest::StatusCode;
use std::time::Duration;
use support::Fixture;
use tokio::time::timeout;

#[tokio::test]
async fn static_index_assets_head_and_spa_are_served_by_the_rust_binary() {
    let fixture = Fixture::new();
    let (_child, base) = fixture.start().await;
    let client = reqwest::Client::new();
    let response = client.get(format!("{base}/")).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(response.headers()["cache-control"], "no-cache");
    assert_eq!(
        response.headers()["cross-origin-opener-policy"],
        "same-origin"
    );
    assert_eq!(
        response.headers()["cross-origin-embedder-policy"],
        "require-corp"
    );
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    let index = response.text().await.unwrap();
    for path in ["/index.html", "/notes/example", "/notes/example/"] {
        let response = client.get(format!("{base}{path}")).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(response.text().await.unwrap(), index);
    }
    let asset = client.get(format!("{base}/app.css")).send().await.unwrap();
    assert_eq!(asset.status(), StatusCode::OK);
    assert_eq!(asset.headers()["cache-control"], "no-cache");
    let length = asset.headers()["content-length"].clone();
    let etag = asset.headers()["etag"].clone();
    let bytes = asset.bytes().await.unwrap();
    let head = client.head(format!("{base}/app.css")).send().await.unwrap();
    assert_eq!(head.status(), StatusCode::OK);
    assert_eq!(head.headers()["content-length"], length);
    assert!(head.bytes().await.unwrap().is_empty());
    let cached = client
        .get(format!("{base}/app.css"))
        .header("if-none-match", etag)
        .send()
        .await
        .unwrap();
    assert_eq!(cached.status(), StatusCode::NOT_MODIFIED);
    let hashed = client
        .get(format!("{base}/app.12345678.css"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        hashed.headers()["cache-control"],
        "public, max-age=31536000, immutable"
    );
    assert_eq!(hashed.bytes().await.unwrap(), bytes);
    let wasm = client.get(format!("{base}/app.wasm")).send().await.unwrap();
    assert_eq!(wasm.headers()["content-type"], "application/wasm");
    assert_eq!(wasm.bytes().await.unwrap().as_ref(), &[0, 97, 115, 109]);
}

#[tokio::test]
async fn static_fallback_preserves_api_authentication_and_missing_asset_boundaries() {
    let fixture = Fixture::new();
    let (_child, base) = fixture.start().await;
    let client = reqwest::Client::new();
    assert_eq!(
        client
            .get(format!("{base}/"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    for path in [
        "/api",
        "/api/v2/vaults",
        "/api/trpc/system.info",
        "/__lapis/missing",
        "/assets/missing",
        "/missing.js",
    ] {
        let response = client.get(format!("{base}{path}")).send().await.unwrap();
        assert!(!response.status().is_success(), "{path}");
        assert!(
            !response.text().await.unwrap().contains("Lapis fixture"),
            "{path}"
        );
    }
    let protected = client
        .get(format!("{base}/api/v3/vaults"))
        .send()
        .await
        .unwrap();
    assert_eq!(protected.status(), StatusCode::UNAUTHORIZED);
    let info = client
        .get(format!("{base}/api/v3/system/info"))
        .send()
        .await
        .unwrap();
    assert_eq!(info.status(), StatusCode::OK);
    let mutate = client
        .post(format!("{base}/notes/example"))
        .body("replace")
        .send()
        .await
        .unwrap();
    assert_eq!(mutate.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        std::fs::read_to_string(fixture.assets.join("index.html")).unwrap(),
        "<!doctype html><title>Lapis fixture</title>"
    );
}

#[tokio::test]
async fn health_and_readiness_match_the_existing_host_without_authentication() {
    let fixture = Fixture::new();
    let (_child, base) = fixture.start().await;
    let client = reqwest::Client::new();
    for (route, status) in [("healthz", "ok"), ("readyz", "ready")] {
        let response = client.get(format!("{base}/{route}")).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(
            response.json::<serde_json::Value>().await.unwrap(),
            serde_json::json!({"status": status})
        );
    }
}

#[tokio::test]
async fn invalid_or_overlapping_operator_build_roots_fail_before_listening() {
    let fixture = Fixture::new();
    let missing = fixture.root.path().join("missing");
    let invalid = fixture.root.path().join("invalid");
    std::fs::create_dir(&invalid).unwrap();
    for root in [
        missing.as_path(),
        invalid.as_path(),
        fixture.vault.as_path(),
        fixture.root.path(),
    ] {
        let mut child = fixture.command(root).spawn().unwrap();
        let exit = timeout(Duration::from_secs(3), child.wait()).await;
        assert!(
            exit.is_ok(),
            "invalid static root started serving: {}",
            root.display()
        );
        assert!(!exit.unwrap().unwrap().success());
    }
}
