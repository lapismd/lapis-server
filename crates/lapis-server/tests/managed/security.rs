use super::{Fixture, ORIGIN};
use serde_json::{Value, json};

#[tokio::test]
async fn management_retains_cookie_csrf_origin_and_typed_name_boundaries() {
    let mut f = Fixture::start().await;
    let client = reqwest::Client::new();
    let url = format!("{}/api/v3/vaults", f.base);
    assert_eq!(
        client
            .post(&url)
            .header("x-lapis-protocol", 3)
            .json(&json!({"name":"Denied"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        f.post("/api/v3/vaults")
            .header("origin", "https://wrong.example")
            .json(&json!({"name":"Denied"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    for name in [
        String::new(),
        "x".repeat(201),
        "💎".repeat(101),
        "Invalid\nname".into(),
    ] {
        assert_eq!(
            f.post("/api/v3/vaults")
                .json(&json!({"name":name}))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    let invalid = f
        .post("/api/v3/vaults")
        .json(&json!({"name":"Invalid", "root":"/tmp/forbidden"}))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 422);
    assert_eq!(
        invalid.json::<Value>().await.unwrap()["code"],
        "INVALID_REQUEST"
    );
    let login = f
        .post("/api/v3/auth/login")
        .header("origin", ORIGIN)
        .json(&json!({"username":"owner","password":crate::credentials::PASSWORD}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let cookie = login.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let info: Value = login.json().await.unwrap();
    let request = || {
        client
            .post(&url)
            .header("cookie", &cookie)
            .header("origin", ORIGIN)
            .header("x-lapis-protocol", 3)
            .json(&json!({"name":"Owned by the installation"}))
    };
    assert_eq!(request().send().await.unwrap().status(), 403);
    assert_eq!(
        request()
            .header("x-lapis-csrf", info["csrf_token"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        201
    );
    f.finish().await;
}

#[tokio::test]
async fn replaced_managed_storage_cannot_create_content_outside_the_profile() {
    use std::os::unix::fs::symlink;
    let mut f = Fixture::start().await;
    let outside = tempfile::tempdir().unwrap();
    let storage = f.profile.join("vaults");
    let retained = f.profile.join("vaults-original");
    std::fs::rename(&storage, &retained).unwrap();
    symlink(outside.path(), &storage).unwrap();
    assert_eq!(
        f.post("/api/v3/vaults")
            .json(&json!({"name":"Must not escape"}))
            .send()
            .await
            .unwrap()
            .status(),
        500
    );
    assert!(outside.path().read_dir().unwrap().next().is_none());
    assert_eq!(f.list().await.len(), 1);
    f.finish().await;
}
