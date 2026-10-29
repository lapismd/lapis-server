use crate::{
    database::Database,
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime, VaultService},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) const ID: &str = "docs/note";
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
/// Case-exclusion fixtures require a filesystem that distinguishes name case;
/// case-insensitive hosts (default macOS APFS) cannot represent them.
pub(super) fn fs_distinguishes_case(root: &tempfile::TempDir) -> bool {
    let lower = root.path().join("case-probe.tmp");
    let upper = root.path().join("CASE-PROBE.tmp");
    std::fs::write(&lower, "probe").unwrap();
    let collided = upper.exists();
    std::fs::remove_file(&lower).unwrap();
    !collided
}
pub(super) fn note(id: &str, title: &str) -> String {
    format!("---\nid: {id}\ntitle: {title}\n---\n# Body\n")
}
pub(super) async fn database(vault: &Arc<VaultService>) -> Database {
    vault.database().await.unwrap()
}
pub(super) async fn row(database: &Database, path: &str) -> serde_json::Value {
    match database.get_projection_row(ID, path, None).await {
        Ok(row) => row.unwrap_or(serde_json::Value::Null),
        Err(error)
            if error.code() == lapis_app_protocol::ErrorCode::InvalidRequest
                && error.0.message == "Projection is not registered" =>
        {
            serde_json::Value::Null
        }
        Err(error) => panic!("Docs row read failed: {error}"),
    }
}
pub(super) async fn until(mut ready: impl AsyncFnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready().await {
        assert!(
            Instant::now() < deadline,
            "native Docs projection did not converge"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
pub(super) async fn rescan(vault: &Arc<VaultService>, root: &str) {
    let before = vault.indexing_status().unwrap().passes;
    vault.start_docs_indexing(root).await.unwrap();
    until(async || vault.indexing_status().unwrap().passes > before).await;
}
pub(super) async fn close(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
