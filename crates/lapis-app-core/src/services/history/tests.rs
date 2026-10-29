use super::*;
use crate::services::{BackupStorage, Runtime};
use serde_json::{Value, json};
use std::{fs, time::Duration};

async fn call(client: &Client, method: HistoryMethod, data: Value) -> Result<Value> {
    let text = client
        .read_history(method, json!({"version":1,"data":data}).to_string())
        .await?;
    Ok(serde_json::from_str::<Value>(&text).unwrap()["data"].clone())
}
async fn close(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
async fn selected(client: &Client) -> String {
    let vault = client.descriptor().vault_id;
    let heads = call(client, HistoryMethod::Heads, json!({"vault":vault}))
        .await
        .unwrap();
    let handle = call(
        client,
        HistoryMethod::Open,
        json!({"vault":vault,"operation":heads["heads"][0]}),
    )
    .await
    .unwrap()
    .as_str()
    .unwrap()
    .to_owned();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let status = call(
                client,
                HistoryMethod::Status,
                json!({"vault":vault,"handle":handle}),
            )
            .await
            .unwrap();
            if status["state"] == "ready" {
                break;
            }
            assert_ne!(status["state"], "failed", "{status}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    handle
}
fn fixture() -> (
    tempfile::TempDir,
    Arc<Runtime>,
    Arc<VaultService>,
    Arc<Client>,
) {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    fs::create_dir(base.join("notes")).unwrap();
    fs::create_dir(base.join("history")).unwrap();
    fs::write(base.join("notes/Note.md"), "saved native Repository text").unwrap();
    let runtime = Arc::new(
        Runtime::default().with_backup(BackupStorage::new(&base.join("history")).unwrap()),
    );
    let vault = runtime.admit(Uuid::new_v4(), &base.join("notes")).unwrap();
    let client = Arc::new(vault.attach().unwrap());
    (temp, runtime, vault, client)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_reader_has_no_capture_initialization_or_cache() {
    let (temp, runtime, vault, client) = fixture();
    let heads = call(&client, HistoryMethod::Heads, json!({"vault":vault.id()}))
        .await
        .unwrap();
    assert_eq!(heads["heads"], json!([]));
    assert!(
        !temp
            .path()
            .join("history")
            .join(vault.id().to_string())
            .exists()
    );
    assert!(!temp.path().join("history/.reader-cache").exists());
    client.detach().unwrap();
    close(runtime).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reader_prepares_existing_history_and_fences_retained_client_after_detach() {
    let (_temp, runtime, vault, client) = fixture();
    client.start_backup().await.unwrap();
    let handle = selected(&client).await;
    let peer = Arc::new(vault.attach().unwrap());
    let peer_handle = selected(&peer).await;
    let read = call(
        &client,
        HistoryMethod::Read,
        json!({"vault":vault.id(),"handle":handle,"query":{"kind":"query_info"}}),
    )
    .await
    .unwrap();
    assert_eq!(read["kind"], "query_info");
    client.detach().unwrap();
    assert!(matches!(
        call(&client, HistoryMethod::Heads, json!({"vault":vault.id()})).await,
        Err(Error::Denied)
    ));
    assert!(client.read_text("Note.md").is_err());
    assert!(
        call(
            &peer,
            HistoryMethod::Status,
            json!({"vault":vault.id(),"handle":peer_handle})
        )
        .await
        .is_ok()
    );
    assert!(vault.backup.lock().unwrap().is_some());
    peer.detach().unwrap();
    close(runtime).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reader_revalidates_physical_source_and_retains_owner_after_failed_close() {
    let (temp, runtime, vault, client) = fixture();
    client.start_backup().await.unwrap();
    let handle = selected(&client).await;
    let owner = vault
        .history_reader
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();
    let root = temp.path().join("notes");
    let retained = temp.path().join("retained");
    fs::rename(&root, &retained).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("Note.md"), "foreign bytes").unwrap();
    assert!(
        call(
            &client,
            HistoryMethod::Status,
            json!({"vault":vault.id(),"handle":handle})
        )
        .await
        .is_err()
    );
    let closing = runtime.clone();
    assert!(
        tokio::task::spawn_blocking(move || closing.shutdown())
            .await
            .unwrap()
            .is_err()
    );
    assert!(Arc::ptr_eq(
        &owner,
        vault.history_reader.lock().unwrap().as_ref().unwrap()
    ));
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "foreign bytes"
    );
    fs::remove_dir_all(&root).unwrap();
    fs::rename(&retained, &root).unwrap();
    assert!(
        call(
            &client,
            HistoryMethod::Status,
            json!({"vault":vault.id(),"handle":handle})
        )
        .await
        .is_ok()
    );
    client.detach().unwrap();
    close(runtime).await;
    assert!(vault.history_reader.lock().unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reader_rejects_foreign_vault_frames_and_request_limits() {
    let (_temp, runtime, vault, client) = fixture();
    assert!("capture".parse::<HistoryMethod>().is_err());
    let oversized = "x".repeat(wire::MAX_REQUEST_BYTES + 1);
    assert!(matches!(
        client.read_history(HistoryMethod::Heads, oversized).await,
        Err(Error::Limit)
    ));
    assert!(matches!(
        client
            .read_history(
                HistoryMethod::Heads,
                json!({"version":2,"data":{"vault":vault.id()}}).to_string()
            )
            .await,
        Err(Error::Invalid)
    ));
    assert!(matches!(
        call(
            &client,
            HistoryMethod::Heads,
            json!({"vault":Uuid::new_v4()})
        )
        .await,
        Err(Error::Denied)
    ));
    client.detach().unwrap();
    close(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reader_rejects_a_foreign_saved_native_binding() {
    let (temp, runtime, vault, client) = fixture();
    client.start_backup().await.unwrap();
    let path = temp
        .path()
        .join("history")
        .join(vault.id().to_string())
        .join("vault.json");
    let original = fs::read(&path).unwrap();
    for field in ["vault_id", "root_identity"] {
        let mut manifest: Value = serde_json::from_slice(&original).unwrap();
        if field == "vault_id" {
            manifest["binding"][field] = json!(Uuid::new_v4());
        } else {
            manifest[field] = json!("foreign-physical-root");
        }
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let peer = vault.attach().unwrap();
        let result = call(&peer, HistoryMethod::Heads, json!({"vault":vault.id()})).await;
        fs::write(&path, &original).unwrap();
        peer.detach().unwrap();
        assert!(
            result.is_err(),
            "Foreign {field} must not supply Repository heads: {result:?}"
        );
    }
    client.detach().unwrap();
    close(runtime).await;
}
