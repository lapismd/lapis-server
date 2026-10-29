use lapis_app_core::database::Database;
use serde_json::json;
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use uuid::Uuid;
mod projection_support;
mod search_support;

#[test]
fn metadata_survives_a_killed_native_owner_and_reopens_in_a_fresh_process() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let launch = |mode: &str| {
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "metadata_process_fixture", "--nocapture"])
            .env("LAPIS_METADATA_TEST_DIRECTORY", profile.path())
            .env("LAPIS_METADATA_TEST_VAULT", id.to_string())
            .env("LAPIS_METADATA_TEST_MODE", mode)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let mut writer = launch("write");
    let output = writer.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            if line.unwrap().contains("METADATA_COMMITTED") {
                sender.send(()).unwrap();
                return;
            }
        }
    });
    let ready = receiver.recv_timeout(Duration::from_secs(10));
    // Always stop this test-owned child, including a readiness failure.
    writer.kill().unwrap();
    writer.wait().unwrap();
    reader.join().unwrap();
    ready.unwrap();
    let mut reader = launch("read");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = reader.try_wait().unwrap() {
            let output = reader.wait_with_output().unwrap();
            assert!(
                status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("METADATA_RESTORED"));
            break;
        }
        if Instant::now() >= deadline {
            reader.kill().unwrap();
            reader.wait().unwrap();
            panic!("Fresh native database owner did not exit");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[tokio::test]
async fn metadata_process_fixture() {
    let Some(directory) = std::env::var_os("LAPIS_METADATA_TEST_DIRECTORY") else {
        return;
    };
    let id = Uuid::parse_str(&std::env::var("LAPIS_METADATA_TEST_VAULT").unwrap()).unwrap();
    let db = Database::open(std::path::Path::new(&directory), id)
        .await
        .unwrap();
    if std::env::var("LAPIS_METADATA_TEST_MODE").unwrap() == "write" {
        db.set_meta("cold", Some(json!({"value":"native 🪨"})))
            .await
            .unwrap();
        let notice = serde_json::from_value(json!({"id":"cold-notice","message":"native notice","severity":"warning","createdAt":1,"updatedAt":1,"read":false,"cleared":false})).unwrap();
        db.upsert_notification(notice).await.unwrap();
        let history=serde_json::from_value(json!({"path":"cold.md","eventType":"create","createdAt":1,"content":"history 🪨","contentHash":"cold-hash"})).unwrap();
        db.store_file_history_revision(history).await.unwrap();
        let indexed=serde_json::from_value(json!({"file":{"path":"cold.md","normalizedPath":"cold.md","extension":"md","mtime":1,"size":10,"hash":"cold-indexed","indexed":true},"metadata":{"path":"cold.md","hash":"cold-indexed","parserVersion":"cold-1","metadata":{"heading":"native"}},"properties":[],"tags":[],"links":[]})).unwrap();
        db.upsert_indexed_metadata(indexed).await.unwrap();
        db.register_projection_definition(
            serde_json::from_value(projection_support::definition("books/book")).unwrap(),
        )
        .await
        .unwrap();
        let mut projection = projection_support::source("books/book", "cold.md", "cold-row");
        projection["sourceHash"] = json!("cold-indexed");
        db.replace_projection_source(serde_json::from_value(projection).unwrap())
            .await
            .unwrap();
        search_support::seed(&db).await;
        println!("METADATA_COMMITTED");
        std::future::pending::<()>().await;
    } else {
        assert_eq!(
            db.get_meta("cold").await.unwrap(),
            Some(json!({"value":"native 🪨"}))
        );
        assert_eq!(db.revision().await.unwrap(), 10);
        let notices = db.list_notifications().await.unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].id, "cold-notice");
        let history = db.get_file_history("cold.md").await.unwrap().unwrap();
        assert_eq!(history.revisions[0].content, "history 🪨");
        assert_eq!(history.revisions[0].content_hash, "cold-hash");
        assert_eq!(
            db.get_indexed_file("cold.md")
                .await
                .unwrap()
                .unwrap()
                .metadata
                .unwrap()
                .parser_version,
            "cold-1"
        );
        assert_eq!(
            db.get_projection_row("books/book", "cold-row", None)
                .await
                .unwrap()
                .unwrap()["title"],
            json!("native 🪨")
        );
        let search = search_support::fixture();
        search_support::assert_json(
            serde_json::to_value(db.list_search_documents().await.unwrap()).unwrap(),
            serde_json::Value::Array(
                search["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r["stored"].clone())
                    .collect(),
            ),
        );
        search_support::assert_json(
            serde_json::to_value(db.get_search_index_stats().await.unwrap()).unwrap(),
            search["stats"].clone(),
        );
        db.close().await.unwrap();
        println!("METADATA_RESTORED");
    }
}
