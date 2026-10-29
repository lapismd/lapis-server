use crate::{
    config::Config,
    control::Control,
    host::{Host, ManagementCommand},
};
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn accepted_catalogue_work_survives_a_cancelled_caller_and_drains_before_shutdown() {
    let profile = tempfile::tempdir().unwrap();
    let control = Arc::new(Control::open(profile.path()).await.unwrap());
    let encoded = tokio::task::spawn_blocking(|| {
        crate::control::passwords::hash("isolated catalogue lifecycle password")
    })
    .await
    .unwrap()
    .unwrap();
    control
        .initialize("owner", &encoded, 1_700_000_000_000)
        .await
        .unwrap();
    let host = Arc::new(Host::with_profile(Config { vaults: vec![] }, profile.path()).unwrap());
    host.enable_management(control.clone(), profile.path())
        .await
        .unwrap();
    let manager = host.management.get().unwrap().clone();
    let gate = manager.gate.lock().await;
    let caller_host = host.clone();
    let caller = tokio::spawn(async move {
        caller_host
            .manage(ManagementCommand::Create(
                "Accepted without a caller".into(),
            ))
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while manager.lifetime.state.lock().unwrap().1 == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    let stopping_host = host.clone();
    let stopping = tokio::task::spawn_blocking(move || stopping_host.shutdown());
    tokio::time::timeout(Duration::from_secs(2), async {
        while !manager.lifetime.state.lock().unwrap().0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        !stopping.is_finished(),
        "shutdown must wait for the accepted catalogue mutation"
    );
    drop(gate);
    tokio::time::timeout(Duration::from_secs(5), stopping)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let rows = control.vaults().await.unwrap();
    assert_eq!(rows.len(), 1);
    let id = rows[0].vault.vault_id;
    assert_eq!(rows[0].vault.name, "Accepted without a caller");
    assert!(profile.path().join(format!("vaults/{id}/content")).is_dir());
    control.close().await.unwrap();
    drop(host);
    drop(manager);
    drop(control);
    let reopened = Control::open(profile.path()).await.unwrap();
    assert_eq!(reopened.vaults().await.unwrap()[0].vault.vault_id, id);
    reopened.close().await.unwrap();
}
