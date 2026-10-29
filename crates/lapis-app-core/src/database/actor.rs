use super::{Completion, Operation, Output, engine, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::changes::ChangeSet;
use tokio::sync::{OwnedSemaphorePermit, broadcast, mpsc, oneshot, watch};

pub(super) struct Envelope {
    pub operation: Operation,
    pub reply: oneshot::Sender<Result<Output>>,
    pub _permit: OwnedSemaphorePermit,
}
pub(super) async fn run(
    mut store: Store,
    mut receiver: mpsc::Receiver<Envelope>,
    done: watch::Sender<Completion>,
    changes: broadcast::Sender<ChangeSet>,
) {
    while let Some(request) = receiver.recv().await {
        // There is intentionally no select on the response receiver's lifetime.
        let result = store.execute(request.operation).await;
        while let Some(change) = store.committed.pop_front() {
            let _ = changes.send(change);
        }
        let _ = request.reply.send(result);
    }
    let result = async {
        let mut rows = store
            .connection
            .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
            .await
            .map_err(engine)?;
        while let Some(row) = rows.next().await.map_err(engine)? {
            if row.get::<i64>(0).map_err(engine)? != 0 {
                return Err(crate::services::Error::new(
                    lapis_app_protocol::ErrorCode::Busy,
                    "Application database checkpoint is busy",
                ));
            }
        }
        Ok(())
    }
    .await;
    // Releasing the connection and native lock precedes the completion signal.
    drop(store);
    drop(changes);
    done.send_replace(Some(result.map_err(|error| error.0)));
}
