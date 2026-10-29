use super::support::Fixture;
use jj_store_jj::vault_workspace::{Binding, HistoryReader};
use std::{fs, time::Duration};

async fn saved_note(binding: &Binding) -> String {
    let reader = HistoryReader::open(binding.clone()).await.unwrap();
    let checkpoint = reader
        .history_revision(None, 0)
        .await
        .unwrap()
        .revision
        .unwrap()
        .checkpoint;
    let files = reader
        .history_files(&checkpoint.commit, None, &[])
        .await
        .unwrap();
    files
        .files
        .into_iter()
        .find(|file| file.path == "Note.md")
        .unwrap()
        .text
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hosted_profile_keeps_native_backup_after_detach_and_drains_final_saved_bytes() {
    let f = Fixture::with_database().await;
    fs::write(f.roots[0].path().join("Note.md"), "initial hosted note").unwrap();
    let attached = f.attach(f.ids[0]).await;
    let binding = Binding {
        vault_id: f.ids[0].to_string(),
        root: f.roots[0].path().canonicalize().unwrap(),
        state: f
            .profile
            .as_ref()
            .unwrap()
            .path()
            .canonicalize()
            .unwrap()
            .join("vault-history")
            .join(f.ids[0].to_string()),
    };
    assert_eq!(saved_note(&binding).await, "initial hosted note");
    f.detach(&attached).await;
    fs::write(f.roots[0].path().join("Note.md"), "saved after HTTP detach").unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        while saved_note(&binding).await != "saved after HTTP detach" {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    fs::write(f.roots[0].path().join("Note.md"), "final hosted bytes").unwrap();
    let host = f.host.clone();
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved_note(&binding).await, "final hosted bytes");
    assert!(!binding.root.join(".jj").exists());
}
