use lapis_app_core::database::{Database, MetaCheck, MetaMutation, MetaScan, MetaTransaction};
use lapis_app_protocol::ErrorCode;
use serde_json::json;
use uuid::Uuid;

fn mutation(key: &str, value: serde_json::Value) -> MetaMutation {
    MetaMutation {
        key: key.into(),
        value: Some(value),
        delete: false,
    }
}

#[tokio::test]
async fn native_metadata_keeps_null_missing_pagination_revisions_and_cold_storage() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(db.get_meta("missing").await.unwrap(), None);
    assert_eq!(db.revision().await.unwrap(), 0);
    db.set_meta("plugin:a", Some(json!({"title":"日本語 🪨","revision":1})))
        .await
        .unwrap();
    db.set_meta("plugin:b", Some(json!(null))).await.unwrap();
    db.set_meta("plugin:c", Some(json!([1, true, "three"])))
        .await
        .unwrap();
    assert_eq!(db.get_meta("plugin:b").await.unwrap(), Some(json!(null)));
    let page = db
        .scan_meta(MetaScan {
            prefix: "plugin:".into(),
            after: None,
            limit: Some(2),
        })
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 2);
    assert_eq!(page.entries[0].key, "plugin:a");
    assert_eq!(page.next_cursor.as_deref(), Some("plugin:b"));
    let rest = db
        .scan_meta(MetaScan {
            prefix: "plugin:".into(),
            after: page.next_cursor,
            limit: None,
        })
        .await
        .unwrap();
    assert_eq!(rest.entries[0].key, "plugin:c");
    assert!(rest.next_cursor.is_none());
    db.set_meta("plugin:b", None).await.unwrap();
    assert_eq!(db.revision().await.unwrap(), 4);
    db.close().await.unwrap();
    assert_eq!(
        db.get_meta("plugin:a").await.unwrap_err().code(),
        ErrorCode::Closed
    );
    let reopened = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(reopened.revision().await.unwrap(), 4);
    assert_eq!(
        reopened.get_meta("plugin:a").await.unwrap(),
        Some(json!({"title":"日本語 🪨","revision":1}))
    );
    assert_eq!(reopened.get_meta("plugin:b").await.unwrap(), None);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn competing_metadata_claims_are_atomic_and_failed_checks_never_advance_revision() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut attempts = Vec::new();
    for owner in 0..24 {
        let db = db.clone();
        attempts.push(tokio::spawn(async move {
            db.transact_meta(MetaTransaction {
                checks: vec![MetaCheck {
                    key: "job:claim".into(),
                    exists: Some(false),
                    value: None,
                }],
                mutations: vec![
                    mutation("job:claim", json!(owner)),
                    mutation("job:status", json!("running")),
                ],
            })
            .await
            .unwrap()
        }));
    }
    let mut applied = 0;
    for attempt in attempts {
        applied += usize::from(attempt.await.unwrap());
    }
    assert_eq!(applied, 1);
    assert_eq!(db.revision().await.unwrap(), 1);
    assert!(
        !db.transact_meta(MetaTransaction {
            checks: vec![MetaCheck {
                key: "job:status".into(),
                exists: None,
                value: Some(json!("finished"))
            }],
            mutations: vec![mutation("job:claim", json!("overwritten"))],
        })
        .await
        .unwrap()
    );
    assert_eq!(db.revision().await.unwrap(), 1);
    assert!(
        db.transact_meta(MetaTransaction {
            checks: vec![],
            mutations: vec![]
        })
        .await
        .unwrap()
    );
    assert_eq!(db.revision().await.unwrap(), 1);
    db.close().await.unwrap();
}

#[tokio::test]
async fn metadata_wire_keeps_explicit_null_separate_from_omitted_values() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let transaction: MetaTransaction = serde_json::from_value(json!({
        "checks": [{"key":"nullable","exists":false}],
        "mutations": [{"key":"nullable","value":null}]
    }))
    .unwrap();
    assert!(db.transact_meta(transaction).await.unwrap());
    assert_eq!(db.get_meta("nullable").await.unwrap(), Some(json!(null)));
    let deletion: MetaTransaction = serde_json::from_value(json!({
        "checks": [{"key":"nullable","value":null}],
        "mutations": [{"key":"nullable"}]
    }))
    .unwrap();
    assert!(db.transact_meta(deletion).await.unwrap());
    assert_eq!(db.get_meta("nullable").await.unwrap(), None);
    assert!(
        serde_json::from_value::<MetaTransaction>(
            json!({"mutations":[],"sql":"DROP TABLE app_meta"})
        )
        .is_err()
    );
    db.close().await.unwrap();
}
