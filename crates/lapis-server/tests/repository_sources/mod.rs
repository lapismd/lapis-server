use super::support::Fixture;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path, process::Command};
use uuid::Uuid;
mod layouts;
mod scopes;
mod security;

fn metadata(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    fn visit(root: &Path, path: &Path, result: &mut BTreeMap<String, Option<Vec<u8>>>) {
        let entry = fs::symlink_metadata(path).unwrap();
        let name = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        if entry.is_dir() {
            result.insert(name, None);
            for child in fs::read_dir(path).unwrap() {
                visit(root, &child.unwrap().path(), result);
            }
        } else {
            result.insert(name, Some(fs::read(path).unwrap()));
        }
    }
    let mut result = BTreeMap::new();
    for name in [".jj", ".git"] {
        let path = root.join(name);
        if path.exists() {
            visit(root, &path, &mut result);
        }
    }
    result
}
fn init(root: &Path) {
    let result = Command::new("jj")
        .args([
            "--config",
            "user.name=\"Source fixture\"",
            "--config",
            "user.email=\"source@example.invalid\"",
            "git",
            "init",
            "--colocate",
        ])
        .arg(root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

async fn sources(f: &Fixture, client: Uuid, kind: &str) -> Value {
    source_response(f, client, json!({"kind":kind}))
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
async fn source_response(f: &Fixture, client: Uuid, value: Value) -> reqwest::Response {
    f.authorize(f.client.post(format!(
        "{}/api/v3/clients/{client}/repository-sources",
        f.base
    )))
    .json(&value)
    .send()
    .await
    .unwrap()
}
async fn history(f: &Fixture, client: Uuid, method: &str, data: Value) -> reqwest::Response {
    f.authorize(f.client.post(format!(
        "{}/api/v3/clients/{client}/history/{method}",
        f.base
    )))
    .body(json!({"version":1,"data":data}).to_string())
    .send()
    .await
    .unwrap()
}
async fn read(f: &Fixture, client: Uuid, method: &str, data: Value) -> Value {
    history(f, client, method, data)
        .await
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["data"]
        .clone()
}
async fn prepared(f: &Fixture, client: Uuid, scope: &Value) -> Value {
    let heads = read(f, client, "heads", json!({"vault":scope})).await;
    let handle = read(
        f,
        client,
        "open",
        json!({"vault":scope,"operation":heads["heads"][0]}),
    )
    .await;
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let status = read(f, client, "status", json!({"vault":scope,"handle":handle})).await;
            if status["state"] == "ready" {
                return;
            }
            assert_ne!(status["state"], "failed", "{status}");
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    handle
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovery_of_paused_original_source_preserves_all_metadata_and_initializes_nothing() {
    let f = Fixture::with_database().await;
    let root = f.roots[0].path();
    fs::create_dir(root.join(".lapis")).unwrap();
    fs::write(
        root.join(".lapis/app.json"),
        r#"{"pluginData":{"repository-history":{"automaticBackup":false}}}"#,
    )
    .unwrap();
    fs::write(root.join("Note.md"), "saved original bytes").unwrap();
    init(root);
    let before = metadata(root);
    let client = f.attach(f.ids[0]).await;
    let response = f
        .authorize(f.client.post(format!(
            "{}/api/v3/clients/{}/repository-sources",
            f.base, client.client_id
        )))
        .json(&json!({"kind":"discover"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "Original discovery must use the admitted native reader boundary"
    );
    let value: Value = response.json().await.unwrap();
    assert_eq!(value, json!({"original":true,"token":null}));
    assert_eq!(metadata(root), before);
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "saved original bytes"
    );
    let history = f.profile.as_ref().unwrap().path().join("vault-history");
    assert!(!history.join(f.ids[0].to_string()).exists());
    assert!(!history.join(".reader-cache").exists());
}
