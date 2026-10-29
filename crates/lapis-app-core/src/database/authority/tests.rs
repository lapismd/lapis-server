use super::*;
use crate::database::{MAX_BYTES, Operation, Output, actor::Envelope, indexed};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::sync::{Semaphore, broadcast, mpsc, watch};

fn database() -> (Database, mpsc::Receiver<Envelope>) {
    let (sender, receiver) = mpsc::channel(8);
    let (_, finished) = watch::channel(None);
    let (_, changes) = broadcast::channel(8);
    let (_, embedding_status) = watch::channel(None);
    (
        Database {
            admission: Arc::new(Mutex::new(Some(sender))),
            authority: Arc::default(),
            bytes: Arc::new(Semaphore::new(MAX_BYTES)),
            finished,
            changes: Arc::new(changes),
            embedding_status,
        },
        receiver,
    )
}
fn call(method: DatabaseMethod, value: serde_json::Value) -> DatabaseCall {
    DatabaseCall {
        method,
        args: vec![DatabaseValue::Value { value }],
    }
}

#[tokio::test]
async fn cancelled_client_waiter_keeps_handover_fenced_until_accepted_write_finishes() {
    let (database, mut actor) = database();
    let client = database.clone();
    let waiter = tokio::spawn(async move {
        client
            .call_from_client(call(DatabaseMethod::DeleteIndexedFile, json!("Note.md")))
            .await
    });
    let accepted = actor.recv().await.unwrap();
    assert!(matches!(
        accepted.operation,
        Operation::Indexed(indexed::Action::Delete(_))
    ));
    waiter.abort();
    let owner = database.clone();
    let (started, start) = tokio::sync::oneshot::channel();
    let selecting = tokio::spawn(async move {
        started.send(()).unwrap();
        owner.claim_native_metadata().await;
    });
    start.await.unwrap();
    assert!(
        !selecting.is_finished(),
        "cancelled waiter cannot retire accepted publication"
    );
    // Configuration remains admitted while the handover waits for generated work.
    let config = database.clone();
    let unrelated = tokio::spawn(async move {
        config
            .call_from_client(DatabaseCall {
                method: DatabaseMethod::SetMeta,
                args: vec![
                    DatabaseValue::Value {
                        value: json!("configuration"),
                    },
                    DatabaseValue::Value { value: json!(true) },
                ],
            })
            .await
    });
    let next = actor.recv().await.unwrap();
    assert!(matches!(next.operation, Operation::Transact(_)));
    next.reply.send(Ok(Output::Applied(true))).ok().unwrap();
    unrelated.await.unwrap().unwrap();
    assert!(!selecting.is_finished());
    accepted
        .reply
        .send(Ok(Output::Indexed(indexed::Value::Done)))
        .ok()
        .unwrap();
    selecting.await.unwrap();
    assert_eq!(
        database
            .call_from_client(call(DatabaseMethod::DeleteIndexedFile, json!("Note.md")))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PermissionDenied
    );
}

#[tokio::test]
async fn ownership_fences_only_claimed_projection_namespaces() {
    let (database, mut actor) = database();
    database.claim_native_metadata().await;
    database.claim_native_projection("docs/note").await;
    let denied = database
        .call_from_client(call(
            DatabaseMethod::DeleteProjectionSource,
            json!("docs/note"),
        ))
        .await
        .unwrap_err();
    assert_eq!(denied.code(), ErrorCode::PermissionDenied);
    // Other projections retain their current admission and argument validation.
    assert_eq!(
        database
            .call_from_client(call(
                DatabaseMethod::DeleteProjectionSource,
                json!("custom/items")
            ))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    database.release_native_projection("docs/note").await;
    let client = database.clone();
    let allowed = tokio::spawn(async move {
        client
            .call_from_client(call(
                DatabaseMethod::UnregisterProjectionDefinition,
                json!("docs/note"),
            ))
            .await
    });
    let accepted = actor.recv().await.unwrap();
    accepted.reply.send(Ok(Output::Done)).ok().unwrap();
    allowed.await.unwrap().unwrap();
}

#[tokio::test]
async fn cancelled_search_waiter_keeps_native_handover_fenced_until_publication_drains() {
    let (database, mut actor) = database();
    let client = database.clone();
    let waiter = tokio::spawn(async move {
        client
            .call_from_client(call(DatabaseMethod::DeleteSearchDocument, json!("Note.md")))
            .await
    });
    let accepted = actor.recv().await.unwrap();
    assert!(matches!(
        accepted.operation,
        Operation::Search(crate::database::search::Action::Delete(_))
    ));
    waiter.abort();
    let owner = database.clone();
    let (started, start) = tokio::sync::oneshot::channel();
    let selecting = tokio::spawn(async move {
        started.send(()).unwrap();
        owner.claim_native_search().await;
    });
    start.await.unwrap();
    assert!(
        !selecting.is_finished(),
        "a dropped response cannot release accepted Search publication"
    );
    accepted
        .reply
        .send(Ok(Output::Search(crate::database::search::Value::Done)))
        .ok()
        .unwrap();
    selecting.await.unwrap();
    assert_eq!(
        database
            .call_from_client(call(DatabaseMethod::DeleteSearchDocument, json!("Note.md")))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::PermissionDenied
    );
}

#[tokio::test]
async fn detached_generated_facade_keeps_accepted_database_admission_through_close() {
    use std::{future::Future, task::Poll, time::Duration};
    let (database, mut actor) = database();
    let mut pending = Box::pin(
        database.call_from_client(call(DatabaseMethod::DeleteSearchDocument, json!("Note.md"))),
    );
    std::future::poll_fn(|context| {
        assert!(pending.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(pending);
    database.fence();
    let accepted = tokio::time::timeout(Duration::from_secs(2), actor.recv())
        .await
        .expect("accepted generated call must reach the existing worker")
        .expect("closing cannot revoke an already admitted generated facade");
    assert!(matches!(
        accepted.operation,
        Operation::Search(crate::database::search::Action::Delete(_))
    ));
    assert_eq!(
        database
            .call_from_client(call(
                DatabaseMethod::DeleteSearchDocument,
                json!("Other.md")
            ))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Closed
    );
    accepted
        .reply
        .send(Ok(Output::Search(crate::database::search::Value::Done)))
        .ok()
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(2), actor.recv())
            .await
            .expect("completed accepted facade must release its admission")
            .is_none()
    );
}
