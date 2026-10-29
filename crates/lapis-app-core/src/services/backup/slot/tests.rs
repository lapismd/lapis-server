use super::*;
use std::time::Duration;
use tokio::sync::oneshot;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn later_status_waits_for_earlier_accepted_work_even_when_its_worker_starts_first() {
    let slot = Slot::default();
    let selection = slot.reserve().unwrap();
    let status = slot.reserve().unwrap();
    let (ready, started) = oneshot::channel();
    let (published, mut results) = oneshot::channel();
    let later = tokio::spawn(async move {
        ready.send(()).unwrap();
        let _turn = status.wait().await.unwrap();
        published.send("status").unwrap();
    });
    started.await.unwrap();
    // The earlier accepted operation has no worker yet. A scheduled later
    // observer cannot publish ahead of it.
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut results)
            .await
            .is_err()
    );
    let accepted = selection.wait().await.unwrap();
    assert!(matches!(
        results.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    drop(accepted);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), results)
            .await
            .unwrap()
            .unwrap(),
        "status"
    );
    later.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_reservations_and_failed_turns_do_not_strand_later_work() {
    let slot = Slot::default();
    let first = slot.reserve().unwrap();
    let cancelled = slot.reserve().unwrap();
    let last = slot.reserve().unwrap();
    drop(cancelled);
    let later = tokio::spawn(async move { last.wait().await.unwrap() });
    let failed: std::result::Result<(), ()> = async {
        let _turn = first.wait().await.unwrap();
        Err(())
    }
    .await;
    assert!(failed.is_err());
    drop(
        tokio::time::timeout(Duration::from_secs(5), later)
            .await
            .unwrap()
            .unwrap(),
    );
}

#[test]
fn waiting_turns_do_not_consume_the_only_blocking_worker() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let slot = Slot::default();
        let first = slot.reserve().unwrap();
        let next = slot.reserve().unwrap();
        let later = tokio::spawn(async move { next.wait().await.unwrap() });
        tokio::task::yield_now().await;
        assert!(
            tokio::time::timeout(Duration::from_secs(5), tokio::task::spawn_blocking(|| true))
                .await
                .unwrap()
                .unwrap()
        );
        drop(first.wait().await.unwrap());
        drop(
            tokio::time::timeout(Duration::from_secs(5), later)
                .await
                .unwrap()
                .unwrap(),
        );
    });
}
