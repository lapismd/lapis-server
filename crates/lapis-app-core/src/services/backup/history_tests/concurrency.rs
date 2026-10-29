use super::*;
use lapis_app_protocol::{VaultPhase, history_index::*};
use std::{fs::OpenOptions, sync::mpsc};

struct PausedBatch(Option<mpsc::SyncSender<()>>);
impl PausedBatch {
    async fn start(f: &Fixture) -> Self {
        let client = f.vault.attach().unwrap();
        client.stop_backup().await.unwrap();
        let vault = f.vault.clone();
        tokio::task::spawn_blocking(move || vault.stop_history_indexing())
            .await
            .unwrap()
            .unwrap();
        let directory = fs::read_dir(f.binding.state.join("history-search-v2"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.join("owner.lock").is_file())
            .unwrap();
        let (release, resume) = mpsc::sync_channel(1);
        let (entered, ready) = mpsc::sync_channel(1);
        *f.vault.history_index_pause.lock().unwrap() = Some(Box::new(move || {
            // Hold the engine's real exclusive lock on the worker stack. The
            // pause is vault-local and exists only in test builds.
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .open(directory.join("owner.lock"))
                .unwrap();
            lock.lock().unwrap();
            entered.send(()).unwrap();
            let _ = resume.recv_timeout(Duration::from_secs(10));
            drop(lock);
        }));
        f.vault.start_history_indexing().unwrap();
        let pause = Self(Some(release));
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        pause
    }
    fn release(&mut self) {
        if let Some(release) = self.0.take() {
            let _ = release.send(());
        }
    }
}
impl Drop for PausedBatch {
    fn drop(&mut self) {
        self.release();
    }
}

fn query() -> HistoryIndexCommand {
    HistoryIndexCommand::Query {
        search: HistoryIndexQuery {
            expression: HistoryExpression::Term {
                field: Some("content".into()),
                value: "rare initial history".into(),
            },
            case_sensitive: false,
            cursor: None,
            limit: 10,
        },
    }
}
fn context(commit: &str) -> HistoryIndexCommand {
    HistoryIndexCommand::Context {
        commit: commit.into(),
        path: "Note.md".into(),
        start: 0,
        end: 1,
    }
}
async fn admitted(
    f: &Fixture,
    request: &tokio::task::JoinHandle<crate::services::Result<HistoryIndexValue>>,
) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while f.vault.status().unwrap().work < 2 && !request.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_query_waits_for_background_batch_and_drains_after_client_detach() {
    let f = fixture(true).await;
    let saved = indexed(&f, "rare initial history").await;
    let mut pause = PausedBatch::start(&f).await;
    let client = Arc::new(f.vault.attach().unwrap());
    let caller = client.clone();
    let mut read = tokio::spawn(async move { caller.control_history_index(query()).await });
    admitted(&f, &read).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(250), &mut read)
            .await
            .is_err(),
        "an admitted query must wait for the active batch instead of reporting its exclusive index busy"
    );
    client.detach().unwrap();
    assert_eq!(f.vault.status().unwrap().clients, 0);
    let vault = f.vault.clone();
    let closing = tokio::task::spawn_blocking(move || vault.close());
    tokio::time::timeout(Duration::from_secs(3), async {
        while f.vault.status().unwrap().phase != VaultPhase::Closing {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        !closing.is_finished(),
        "close must drain admitted batch and query"
    );
    pause.release();
    let HistoryIndexValue::Page(page) = read.await.unwrap().unwrap() else {
        panic!("expected saved History page");
    };
    assert_eq!(
        page.hits[0].record.revision.checkpoint.commit,
        saved.hits[0].record.revision.checkpoint.commit
    );
    assert_eq!(page.hits[0].record.path.as_deref(), Some("Note.md"));
    tokio::time::timeout(Duration::from_secs(5), closing)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(f.vault.status().unwrap().phase, VaultPhase::Closed);
    assert_eq!(
        fs::read_to_string(f.binding.root.join("Note.md")).unwrap(),
        "rare initial history text"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_context_waits_for_background_batch_and_revalidates_its_source() {
    let f = fixture(true).await;
    let saved = indexed(&f, "rare initial history").await;
    let mut pause = PausedBatch::start(&f).await;
    let client = Arc::new(f.vault.attach().unwrap());
    for command in [
        HistoryIndexCommand::Start {},
        HistoryIndexCommand::Status {},
    ] {
        tokio::time::timeout(
            Duration::from_secs(1),
            client.control_history_index(command),
        )
        .await
        .unwrap()
        .unwrap();
    }
    let caller = client.clone();
    let command = context(&saved.hits[0].record.revision.checkpoint.commit);
    let mut read = tokio::spawn(async move { caller.control_history_index(command).await });
    admitted(&f, &read).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(250), &mut read)
            .await
            .is_err(),
        "saved context must share batch admission and its physical source fence"
    );
    let original = f.binding.root.with_extension("retained");
    fs::rename(&f.binding.root, &original).unwrap();
    fs::create_dir(&f.binding.root).unwrap();
    fs::write(f.binding.root.join("Note.md"), "unadmitted replacement").unwrap();
    pause.release();
    assert!(
        read.await.unwrap().is_err(),
        "waiting reads must revalidate the source after admission"
    );
    assert_eq!(
        fs::read_to_string(f.binding.root.join("Note.md")).unwrap(),
        "unadmitted replacement"
    );
    fs::remove_dir_all(&f.binding.root).unwrap();
    fs::rename(original, &f.binding.root).unwrap();
    drop(client);
    close(&f).await;
}
