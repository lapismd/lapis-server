use super::*;
use std::time::Duration;
use std::time::Instant;

fn wait_pass(owner: &Owner, target: u64) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while owner.status.lock().unwrap().passes < target {
        assert!(
            Instant::now() < deadline,
            "closed backup notification must wake before the five-second retry"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn closed_backup_wakes_once(final_publication: bool) {
    let status = |phase| jj_vault::Status {
        phase,
        checkpoint: None,
        error: None,
    };
    let (publish, receive) = tokio::sync::watch::channel(status(jj_vault::Phase::Ready));
    let owner = Owner::start(Weak::new(), Some(receive)).unwrap();
    // An unavailable source uses the existing five-second retry. A notification
    // must interrupt that wait without a renderer request or periodic repair.
    wait_pass(&owner, 1);
    if final_publication {
        publish.send_replace(status(jj_vault::Phase::Closed));
    }
    drop(publish);
    wait_pass(&owner, 2);
    owner.finish().unwrap();
    assert_eq!(owner.status.lock().unwrap().passes, 2);
    assert!(
        owner.backup.lock().unwrap().is_none(),
        "consume sender closure once"
    );
}

#[test]
fn final_backup_publication_wakes_indexing_after_the_status_sender_closes() {
    closed_backup_wakes_once(true);
}

#[test]
fn closing_an_unchanged_backup_status_also_wakes_indexing_once() {
    closed_backup_wakes_once(false);
}

#[test]
fn last_owner_dropped_on_its_worker_signals_stop_without_joining_itself() {
    // A batch can hold the final upgraded vault Arc after its Runtime is
    // abandoned. Its final release then drops the index owner on this worker.
    let (publish, receive) = mpsc::sync_channel::<Owner>(1);
    let (retired, complete) = mpsc::sync_channel(1);
    let (stop, stopped) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        drop(receive.recv().unwrap());
        retired.send(()).unwrap();
    });
    publish
        .send(Owner {
            status: Arc::new(Mutex::new(HistoryIndexStatus::default())),
            stop,
            backup: Arc::new(Mutex::new(None)),
            requested: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(Some(worker)),
        })
        .unwrap_or_else(|_| panic!("worker must receive its final owner"));
    stopped.recv_timeout(Duration::from_secs(2)).unwrap();
    complete
        .recv_timeout(Duration::from_secs(2))
        .expect("owner must not join its own thread");
}
