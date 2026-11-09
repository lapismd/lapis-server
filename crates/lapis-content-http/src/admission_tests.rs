use super::*;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[tokio::test]
async fn saturation_refuses_work_before_dispatch() {
    let jobs = Arc::new(Semaphore::new(1));
    let _held = jobs.clone().acquire_owned().await.unwrap();
    let called = Arc::new(AtomicBool::new(false));
    let worker = called.clone();
    let error = run(jobs, move || {
        worker.store(true, Ordering::Release);
        Ok(())
    })
    .await
    .unwrap_err();
    assert_eq!(
        error.response(uuid::Uuid::new_v4()).status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(!called.load(Ordering::Acquire));
}

#[tokio::test]
async fn cancellation_retains_capacity_until_blocking_work_finishes() {
    let jobs = Arc::new(Semaphore::new(1));
    let worker_jobs = jobs.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, waiting) = std::sync::mpsc::channel();
    let request = tokio::spawn(run(worker_jobs, move || {
        started.send(()).unwrap();
        waiting.recv_timeout(Duration::from_secs(5)).unwrap();
        Ok(())
    }));
    tokio::time::timeout(Duration::from_secs(5), ready)
        .await
        .unwrap()
        .unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    assert!(jobs.try_acquire().is_err());
    release.send(()).unwrap();
    let _released = tokio::time::timeout(Duration::from_secs(5), jobs.acquire())
        .await
        .unwrap()
        .unwrap();
}
