use crate::{
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime, VaultService},
};
use lapis_app_protocol::database::call::{DatabaseCall, DatabaseMethod, DatabaseValue};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub(super) fn runtime(profile: &tempfile::TempDir) -> Arc<Runtime> {
    Arc::new(Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    ))
}
pub(super) fn write(root: &tempfile::TempDir, path: &str, text: &str) {
    let path = root.path().join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
pub(super) fn document(id: &str, kind: &str, body: &str) -> String {
    format!("---\ntype: {kind}\nid: {id}\n---\n\n# {id}\n\n{body}\n")
}
pub(super) async fn call(
    vault: &Arc<VaultService>,
    method: DatabaseMethod,
    args: Vec<Value>,
) -> crate::services::Result<Value> {
    let work = vault.begin_work()?;
    Ok(
        match work
            .call_database(DatabaseCall {
                method,
                args: args
                    .into_iter()
                    .map(|value| DatabaseValue::Value { value })
                    .collect(),
            })
            .await?
        {
            DatabaseValue::Value { value } => value,
            DatabaseValue::Undefined {} => Value::Null,
        },
    )
}
pub(super) async fn query(vault: &Arc<VaultService>) -> crate::services::Result<Value> {
    call(
        vault,
        DatabaseMethod::QueryProjection,
        vec![json!("tasks/task")],
    )
    .await
}
pub(super) async fn row(vault: &Arc<VaultService>, path: &str) -> Value {
    call(
        vault,
        DatabaseMethod::GetTaskRow,
        vec![json!({"path":path})],
    )
    .await
    .unwrap()
}
pub(super) async fn until(mut ready: impl AsyncFnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready().await {
        assert!(
            Instant::now() < deadline,
            "native Task reconciliation did not complete"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
pub(super) async fn rescan(vault: &Arc<VaultService>, root: &tempfile::TempDir) {
    let before = vault.indexing_status().unwrap().passes;
    write(root, "signal.txt", &Uuid::new_v4().to_string());
    until(async || vault.indexing_status().unwrap().passes > before).await;
}
pub(super) async fn close(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
