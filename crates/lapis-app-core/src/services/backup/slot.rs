use super::{Owner, failure};
use crate::services::{Result, Work};
use std::{
    collections::BTreeSet,
    sync::{Arc, LockResult, Mutex, MutexGuard},
};
use tokio::sync::{Notify, oneshot};
#[cfg(test)]
mod tests;

#[derive(Default)]
struct Sequence {
    issued: u64,
    completed: u64,
    finished: BTreeSet<u64>,
}
#[derive(Default)]
struct Order {
    state: Mutex<Sequence>,
    changed: Notify,
}

/// The same backup owner, with accepted-call order independent of worker
/// scheduling. Reserving a turn never initializes or touches repository data.
#[derive(Default)]
pub(in crate::services) struct Slot {
    value: Mutex<Option<Arc<Owner>>>,
    order: Arc<Order>,
}
pub(super) struct Turn {
    order: Arc<Order>,
    sequence: u64,
}
impl Slot {
    pub fn lock(&self) -> LockResult<MutexGuard<'_, Option<Arc<Owner>>>> {
        self.value.lock()
    }
    /// Reserve synchronously before an accepted operation yields to its worker.
    pub(super) fn reserve(&self) -> Result<Turn> {
        let mut state = self.order.state.lock().map_err(|_| failure())?;
        let sequence = state.issued;
        state.issued = sequence.checked_add(1).ok_or_else(failure)?;
        Ok(Turn {
            order: self.order.clone(),
            sequence,
        })
    }
}
impl Turn {
    /// Native work owns the reservation independently of the response waiter.
    async fn wait(self) -> Result<Self> {
        loop {
            let changed = self.order.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let ready = self.order.state.lock().map_err(|_| failure())?.completed == self.sequence;
            if ready {
                break;
            }
            changed.await;
        }
        Ok(self)
    }
    pub async fn run<T: Send + 'static>(
        self,
        work: Work,
        action: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (reply, response) = oneshot::channel();
        tokio::spawn(async move {
            let result = async move {
                let turn = self.wait().await?;
                tokio::task::spawn_blocking(move || {
                    let _work = work;
                    let _turn = turn;
                    action()
                })
                .await
                .map_err(|_| failure())?
            }
            .await;
            let _ = reply.send(result);
        });
        response.await.map_err(|_| failure())?
    }
}
impl Drop for Turn {
    fn drop(&mut self) {
        // Also retire an unused reservation after an early failure; a gap must
        // never strand later accepted operations. This is order-only cleanup.
        let mut state = self
            .order
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.finished.insert(self.sequence);
        loop {
            let completed = state.completed;
            if !state.finished.remove(&completed) {
                break;
            }
            state.completed += 1;
        }
        self.order.changed.notify_waiters();
    }
}
