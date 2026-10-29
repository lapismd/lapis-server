use super::{Arguments, Database, done, optional, value};
use crate::services::Result;
use lapis_app_protocol::database::call::{DatabaseMethod, DatabaseMethod::*, DatabaseValue};

pub(super) async fn call(
    db: &Database,
    method: DatabaseMethod,
    a: &Arguments,
) -> Result<DatabaseValue> {
    match method {
        Migrate => done(db.migrate().await),
        BeginSearchIndexingBatch => done(db.begin_search_indexing_batch().await),
        EndSearchIndexingBatch => done(db.end_search_indexing_batch().await),
        GetMeta => optional(db.get_meta(a.text(0)?).await),
        SetMeta => done(db.set_meta(a.text(0)?, a.raw(1).cloned()).await),
        ScanMeta => value(db.scan_meta(a.required(0)?).await),
        TransactMeta => value(
            db.transact_meta(a.required(0)?)
                .await
                .map(|applied| serde_json::json!({"applied":applied})),
        ),
        GetNotebookState => optional(db.get_notebook_state(a.text(0)?).await),
        SetNotebookState => done(db.set_notebook_state(a.text(0)?, a.required(1)?).await),
        DeleteNotebookState => done(db.delete_notebook_state(a.text(0)?).await),
        LoadMetadataSnapshot => value(db.load_metadata_snapshot().await),
        SaveMetadataSnapshot => done(db.save_metadata_snapshot(a.required(0)?).await),
        GetFileHistory => value(db.get_file_history(a.text(0)?).await),
        StoreFileHistoryRevision => value(db.store_file_history_revision(a.required(0)?).await),
        ListNotifications => value(db.list_notifications().await),
        UpsertNotification => done(db.upsert_notification(a.required(0)?).await),
        MarkNotificationRead => done(db.mark_notification_read(a.text(0)?).await),
        ClearNotification => done(db.clear_notification(a.text(0)?).await),
        ClearAllNotifications => done(db.clear_all_notifications().await),
        GetChangeRevision => value(db.revision().await),
        _ => unreachable!("call router selects the state domain"),
    }
}
