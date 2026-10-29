use crate::{
    database::Database,
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime, VaultService},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

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
pub(super) async fn select(vault: &Arc<VaultService>, enabled: bool) {
    if enabled {
        vault.start_search_indexing().await.unwrap();
    } else {
        vault.stop_search_indexing().unwrap();
    }
}
pub(super) async fn database(vault: &Arc<VaultService>) -> Database {
    vault.database().await.unwrap()
}
pub(super) async fn until(mut ready: impl AsyncFnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready().await {
        assert!(Instant::now() < deadline, "native Search did not converge");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
pub(super) async fn rescan(vault: &Arc<VaultService>) {
    let passes = vault.indexing_status().unwrap().passes;
    select(vault, true).await;
    until(async || vault.indexing_status().unwrap().passes > passes).await;
}
pub(super) async fn close(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
