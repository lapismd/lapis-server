use super::*;
use lapis_app_protocol::{ErrorCode, history_index::*};

fn query(text: &str) -> HistoryIndexCommand {
    HistoryIndexCommand::Query {
        search: HistoryIndexQuery {
            expression: HistoryExpression::Term {
                field: Some("content".into()),
                value: text.into(),
            },
            case_sensitive: false,
            cursor: None,
            limit: 100,
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_controls_keep_saved_context_when_capture_is_paused() {
    let f = fixture(true).await;
    let saved = indexed(&f, "rare initial history").await;
    let client = f.vault.attach().unwrap();
    client.stop_backup().await.unwrap();
    fs::write(f.binding.root.join("Note.md"), "unsaved private content").unwrap();
    let original = owner(&f);
    let context = client
        .control_history_index(HistoryIndexCommand::Context {
            commit: saved.hits[0].record.revision.checkpoint.commit.clone(),
            path: "Note.md".into(),
            start: 0,
            end: 1,
        })
        .await
        .unwrap();
    let HistoryIndexValue::Context(context) = context else {
        panic!("expected saved context");
    };
    assert_eq!(context.text, "rare initial history text");
    assert!(
        matches!(client.control_history_index(query("unsaved private content")).await.unwrap(), HistoryIndexValue::Page(page) if page.hits.is_empty())
    );
    client
        .control_history_index(HistoryIndexCommand::Start {})
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&original, &owner(&f)));
    assert!(f.vault.backup.lock().unwrap().is_none());
    drop(client);
    close(&f).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_controls_missing_source_stays_empty_without_an_owner_or_repository() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("notes");
    let history = base.join("history");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&history).unwrap();
    fs::write(root.join("Note.md"), "never saved").unwrap();
    let runtime = Arc::new(
        Runtime::default().with_backup(
            BackupStorage::new(&history)
                .unwrap()
                .with_history_indexing(true),
        ),
    );
    let vault = runtime.admit(Uuid::new_v4(), &root).unwrap();
    let client = vault.attach().unwrap();
    for command in [
        HistoryIndexCommand::Status {},
        HistoryIndexCommand::Start {},
        query("never saved"),
    ] {
        let value = client.control_history_index(command).await.unwrap();
        match value {
            HistoryIndexValue::Status(status) => assert!(status.complete),
            HistoryIndexValue::Page(page) => assert!(page.status.complete && page.hits.is_empty()),
            _ => panic!("missing source returned content"),
        }
    }
    let failure = client
        .control_history_index(HistoryIndexCommand::Context {
            commit: "abcd".into(),
            path: "Note.md".into(),
            start: 0,
            end: 1,
        })
        .await
        .unwrap_err();
    assert_eq!(failure.code(), ErrorCode::NotFound);
    assert!(vault.backup.lock().unwrap().is_none());
    assert!(vault.history_indexing.lock().unwrap().is_none());
    assert_eq!(fs::read_dir(&history).unwrap().count(), 0);
    assert!(!root.join(".git").exists() && !root.join(".jj").exists());
    drop(client);
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
