use lapis_app_core::database::{Database, MetaMutation, MetaScan, MetaTransaction};
use lapis_app_protocol::ErrorCode;
use serde_json::json;
use std::{future::Future, task::Poll};
use uuid::Uuid;

#[tokio::test]
async fn accepted_metadata_write_outlives_a_cancelled_waiter_and_close_fences_every_clone() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    let observer = db.clone();
    // Current-thread runtime: the worker cannot run until this poll yields.
    let mut write = Box::pin(db.set_meta("accepted", Some(json!("survives detach"))));
    std::future::poll_fn(|cx| {
        assert!(write.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(write);
    let mut close = Box::pin(db.close());
    std::future::poll_fn(|cx| {
        assert!(close.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(close);
    assert_eq!(
        observer
            .set_meta("late", Some(json!(true)))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Closed
    );
    observer.close().await.unwrap();
    let reopened = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(
        reopened.get_meta("accepted").await.unwrap(),
        Some(json!("survives detach"))
    );
    assert_eq!(reopened.get_meta("late").await.unwrap(), None);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn dropping_last_client_drains_and_releases_native_database_ownership() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    match Database::open(profile.path(), id).await {
        Err(error) => assert_eq!(error.code(), ErrorCode::Busy),
        Ok(_) => panic!("A second worker must not open the same database"),
    }
    let mut write = Box::pin(db.set_meta("accepted", Some(json!(true))));
    std::future::poll_fn(|cx| {
        assert!(write.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(write);
    drop(db);
    let reopened = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match Database::open(profile.path(), id).await {
                Ok(db) => break db,
                Err(error) if error.code() == ErrorCode::Busy => tokio::task::yield_now().await,
                Err(error) => panic!("{error}"),
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(
        reopened.get_meta("accepted").await.unwrap(),
        Some(json!(true))
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn queue_and_byte_bounds_reject_before_acceptance_and_admitted_writes_still_drain() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut pending = Vec::new();
    for _ in 0..64 {
        let mut operation = Box::pin(db.set_meta("queue", Some(json!(true))));
        std::future::poll_fn(|cx| {
            assert!(operation.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        pending.push(operation);
    }
    assert_eq!(
        db.set_meta("overflow", Some(json!(true)))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    drop(pending);
    db.close().await.unwrap();

    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let too_large = json!("x".repeat(32 * 1024 * 1024));
    assert_eq!(
        db.set_meta("large", Some(too_large))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    let invalid = MetaTransaction {
        checks: vec![],
        mutations: (0..1025)
            .map(|_| MetaMutation {
                key: "k".into(),
                value: None,
                delete: true,
            })
            .collect(),
    };
    assert_eq!(
        db.transact_meta(invalid).await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(db.revision().await.unwrap(), 0);
    db.close().await.unwrap();
}

#[tokio::test]
async fn metadata_pagination_bounds_and_literal_keys_do_not_expose_sql_or_patterns() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    db.transact_meta(MetaTransaction {
        checks: vec![],
        mutations: (0..505)
            .map(|n| MetaMutation {
                key: format!("plugin_%:{n:03}"),
                value: Some(json!(n)),
                delete: false,
            })
            .collect(),
    })
    .await
    .unwrap();
    db.set_meta("pluginXY:wrong", Some(json!("excluded")))
        .await
        .unwrap();
    let query = |limit| MetaScan {
        prefix: "plugin_%:".into(),
        after: None,
        limit: Some(limit),
    };
    assert_eq!(db.scan_meta(query(0)).await.unwrap().entries.len(), 1);
    assert_eq!(db.scan_meta(query(999)).await.unwrap().entries.len(), 500);
    let key = "'; DROP TABLE app_meta; --";
    db.set_meta(key, Some(json!(true))).await.unwrap();
    assert_eq!(db.get_meta(key).await.unwrap(), Some(json!(true)));
    db.close().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn native_profile_rejects_symlink_databases_and_lockfiles_and_invalid_identity() {
    use std::os::unix::fs::symlink;
    let profile = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    for suffix in ["db", "lock"] {
        let id = Uuid::new_v4();
        symlink(
            outside.path(),
            profile.path().join(format!("{id}.{suffix}")),
        )
        .unwrap();
        assert!(Database::open(profile.path(), id).await.is_err());
    }
    assert!(Database::open(profile.path(), Uuid::nil()).await.is_err());
    assert!(
        Database::open(std::path::Path::new("relative"), Uuid::new_v4())
            .await
            .is_err()
    );
    assert_eq!(std::fs::metadata(outside.path()).unwrap().len(), 0);
}
