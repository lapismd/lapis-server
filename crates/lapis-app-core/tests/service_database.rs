use lapis_app_core::{
    database::Database,
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime},
};
use lapis_app_protocol::{
    ErrorCode, VaultPhase,
    database::call::{DatabaseCall, DatabaseMethod, DatabaseValue},
};
use serde_json::{Value, json};
use std::{future::Future, sync::Arc, task::Poll, time::Duration};
use uuid::Uuid;

fn selected(profile: &tempfile::TempDir) -> Arc<Runtime> {
    Arc::new(Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    ))
}
fn call(method: DatabaseMethod, args: Vec<Value>) -> DatabaseCall {
    DatabaseCall {
        method,
        args: args
            .into_iter()
            .map(|value| DatabaseValue::Value { value })
            .collect(),
    }
}
fn get(key: &str) -> DatabaseCall {
    call(DatabaseMethod::GetMeta, vec![json!(key)])
}
fn set(key: &str, value: Value) -> DatabaseCall {
    call(DatabaseMethod::SetMeta, vec![json!(key), value])
}
fn value(result: DatabaseValue) -> Value {
    match result {
        DatabaseValue::Value { value } => value,
        DatabaseValue::Undefined {} => panic!("Expected a stored value"),
    }
}

#[tokio::test]
async fn selected_database_is_lazy_shared_and_survives_client_detach() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = selected(&profile);
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, root.path()).unwrap();
    assert_eq!(std::fs::read_dir(profile.path()).unwrap().count(), 0);
    let a = vault.attach().unwrap();
    let b = vault.attach().unwrap();
    let (first, second) = tokio::join!(
        a.call_database(set("first", json!(1))),
        b.call_database(set("second", json!(2))),
    );
    first.unwrap();
    second.unwrap();
    assert_eq!(
        Database::open(profile.path(), id)
            .await
            .err()
            .unwrap()
            .code(),
        ErrorCode::Busy
    );
    drop(a);
    drop(b);
    assert_eq!(vault.status().unwrap().clients, 0);
    let native = vault.begin_work().unwrap();
    native
        .call_database(set("native", json!(true)))
        .await
        .unwrap();
    drop(native);
    let reconnect = vault.attach().unwrap();
    assert_eq!(
        value(reconnect.call_database(get("native")).await.unwrap()),
        true
    );
    assert_eq!(
        value(reconnect.call_database(get("first")).await.unwrap()),
        1
    );
    assert_eq!(
        value(reconnect.call_database(get("second")).await.unwrap()),
        2
    );
    tokio::task::spawn_blocking(move || runtime.close_vault(id))
        .await
        .unwrap()
        .unwrap();
    let reopened = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(reopened.get_meta("first").await.unwrap(), Some(json!(1)));
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn vault_close_fences_database_and_drains_a_cancelled_write_waiter() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = selected(&profile);
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, root.path()).unwrap();
    let client = vault.attach().unwrap();
    client.call_database(get("accepted")).await.unwrap();
    let mut write = Box::pin(client.call_database(set("accepted", json!("persisted"))));
    std::future::poll_fn(|cx| {
        assert!(write.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(write);
    let closing = tokio::task::spawn_blocking(move || runtime.close_vault(id));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while vault.status().unwrap().phase == VaultPhase::Open {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closing);
    assert_eq!(
        client
            .call_database(set("late", json!(true)))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Closed
    );
    closing.await.unwrap().unwrap();
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
    let reopened = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(
        reopened.get_meta("accepted").await.unwrap(),
        Some(json!("persisted"))
    );
    assert_eq!(reopened.get_meta("late").await.unwrap(), None);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn dirty_shutdown_keeps_all_databases_live_then_drains_them_together() {
    let profile = tempfile::tempdir().unwrap();
    let roots = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    std::fs::write(roots[0].path().join("note.md"), "dirty").unwrap();
    let runtime = selected(&profile);
    let a = runtime.admit(Uuid::new_v4(), roots[0].path()).unwrap();
    let b = runtime.admit(Uuid::new_v4(), roots[1].path()).unwrap();
    let first = a.attach().unwrap();
    let second = b.attach().unwrap();
    first.call_database(set("vault", json!("a"))).await.unwrap();
    second
        .call_database(set("vault", json!("b")))
        .await
        .unwrap();
    let editor = first.open_editor("note.md").unwrap();
    first.mark_dirty(editor, 1).unwrap();
    drop(first);
    assert_eq!(runtime.shutdown().unwrap_err().code(), ErrorCode::Busy);
    assert_eq!(a.status().unwrap().phase, VaultPhase::Open);
    assert_eq!(
        value(second.call_database(get("vault")).await.unwrap()),
        "b"
    );
    let reconnect = a.attach().unwrap();
    reconnect.recover_editor(editor).unwrap();
    reconnect.discard_editor(editor).unwrap();
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
    for (vault, expected) in [(a, "a"), (b, "b")] {
        assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
        let database = Database::open(profile.path(), vault.id()).await.unwrap();
        assert_eq!(
            database.get_meta("vault").await.unwrap(),
            Some(json!(expected))
        );
        database.close().await.unwrap();
    }
}

#[tokio::test]
async fn cancelled_lazy_open_does_not_strand_vault_close() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = selected(&profile);
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, root.path()).unwrap();
    let client = vault.attach().unwrap();
    let mut opening = Box::pin(client.call_database(get("missing")));
    std::future::poll_fn(|cx| {
        assert!(opening.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(opening);
    tokio::time::timeout(
        Duration::from_secs(5),
        tokio::task::spawn_blocking(move || runtime.close_vault(id)),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
    let database = Database::open(profile.path(), id).await.unwrap();
    database.close().await.unwrap();
}

#[tokio::test]
async fn file_only_runtime_never_creates_a_database_fallback() {
    let runtime = Runtime::default();
    let root = tempfile::tempdir().unwrap();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    assert_eq!(
        vault
            .attach()
            .unwrap()
            .call_database(get("none"))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Unsupported
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    runtime.shutdown().unwrap();
}

#[test]
fn database_storage_is_native_and_cannot_live_inside_a_vault() {
    let root = tempfile::tempdir().unwrap();
    let profile = root.path().join("profile");
    std::fs::create_dir(&profile).unwrap();
    let runtime = Runtime::with_database(
        DatabaseStorage::new(&profile, NativeEmbeddingHost::default()).unwrap(),
    );
    assert_eq!(
        runtime
            .admit(Uuid::new_v4(), root.path())
            .err()
            .unwrap()
            .code(),
        ErrorCode::PermissionDenied
    );
    assert!(
        DatabaseStorage::new(
            std::path::Path::new("relative"),
            NativeEmbeddingHost::default()
        )
        .is_err()
    );
    assert_eq!(std::fs::read_dir(&profile).unwrap().count(), 0);
}

#[tokio::test]
async fn replaced_vault_or_profile_cannot_open_or_reuse_the_database() {
    for (replace_profile, already_open) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("vault");
        let profile = fixture.path().join("profile");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&profile).unwrap();
        let runtime = Runtime::with_database(
            DatabaseStorage::new(&profile, NativeEmbeddingHost::default()).unwrap(),
        );
        let vault = runtime.admit(Uuid::new_v4(), &root).unwrap();
        let client = vault.attach().unwrap();
        if already_open {
            client
                .call_database(set("original", json!(true)))
                .await
                .unwrap();
        }
        let target = if replace_profile { &profile } else { &root };
        std::fs::rename(target, fixture.path().join("original")).unwrap();
        std::fs::create_dir(target).unwrap();
        assert!(
            client
                .call_database(set("unsafe", json!(true)))
                .await
                .is_err()
        );
        if replace_profile || !already_open {
            assert_eq!(std::fs::read_dir(&profile).unwrap().count(), 0);
        }
        tokio::task::spawn_blocking(move || runtime.shutdown())
            .await
            .unwrap()
            .unwrap();
        if replace_profile {
            assert_eq!(std::fs::read_dir(&profile).unwrap().count(), 0);
        }
    }
}

#[tokio::test]
async fn dropping_the_service_cannot_leak_an_admission_sender_through_its_observer() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    {
        let runtime = selected(&profile);
        let vault = runtime.admit(id, root.path()).unwrap();
        vault
            .attach()
            .unwrap()
            .call_database(set("saved", json!(true)))
            .await
            .unwrap();
    }
    let database = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match Database::open(profile.path(), id).await {
                Ok(database) => break database,
                Err(error) if error.code() == ErrorCode::Busy => tokio::task::yield_now().await,
                Err(error) => panic!("{error}"),
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(database.get_meta("saved").await.unwrap(), Some(json!(true)));
    database.close().await.unwrap();
}

#[tokio::test]
async fn failed_lazy_open_releases_its_slot_without_selecting_another_writer() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = selected(&profile);
    let id = Uuid::new_v4();
    let other_owner = Database::open(profile.path(), id).await.unwrap();
    let vault = runtime.admit(id, root.path()).unwrap();
    let client = vault.attach().unwrap();
    assert_eq!(
        client
            .call_database(set("saved", json!(true)))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    assert_eq!(other_owner.get_meta("saved").await.unwrap(), None);
    other_owner.close().await.unwrap();
    client
        .call_database(set("saved", json!(true)))
        .await
        .unwrap();
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn reopening_uses_a_fresh_service_generation_and_the_same_persisted_database() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = selected(&profile);
    let id = Uuid::new_v4();
    let original = runtime.admit(id, root.path()).unwrap();
    let stale = original.attach().unwrap();
    stale
        .call_database(set("saved", json!(true)))
        .await
        .unwrap();
    let closing = runtime.clone();
    tokio::task::spawn_blocking(move || closing.close_vault(id))
        .await
        .unwrap()
        .unwrap();
    let reopened = runtime.admit(id, root.path()).unwrap();
    let client = reopened.attach().unwrap();
    assert_ne!(
        stale.descriptor().generation,
        client.descriptor().generation
    );
    assert_eq!(
        value(client.call_database(get("saved")).await.unwrap()),
        true
    );
    assert_eq!(
        stale.call_database(get("saved")).await.unwrap_err().code(),
        ErrorCode::Closed
    );
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn a_completed_but_unpolled_client_reply_cannot_hold_vault_close_open() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = selected(&profile);
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, root.path()).unwrap();
    let client = vault.attach().unwrap();
    client.call_database(get("saved")).await.unwrap();
    let mut reply = Box::pin(client.call_database(set("saved", json!(true))));
    std::future::poll_fn(|context| {
        assert!(reply.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    // The second operation proves publication without polling the first waiter.
    assert_eq!(
        value(client.call_database(get("saved")).await.unwrap()),
        true
    );
    let close = tokio::task::spawn_blocking(move || runtime.close_vault(id));
    tokio::time::timeout(Duration::from_secs(5), close)
        .await
        .expect("client response consumption cannot own native close")
        .unwrap()
        .unwrap();
    reply.await.unwrap();
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
}
