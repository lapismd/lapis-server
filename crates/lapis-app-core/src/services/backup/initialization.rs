//! Opt-in, bounded debug diagnostics. Never format native errors or paths.
use super::failure;
use crate::services::Result;
use std::{
    io::{self, Write},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

static NEXT_ATTEMPT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug)]
pub(super) enum Stage {
    ClientAdmission,
    WorkReservation,
    QueueReservation,
    SelectStorage,
    ValidateStorage,
    LockOwner,
    ReadConfiguration,
    Admission,
    Executor,
    RecoverStorage,
    ResolveBinding,
    Revalidate,
    StartWorker,
    CreateWatcher,
    WatchRoot,
    InitialCapture,
    HistoryIndexing,
}

pub(super) struct Initialization {
    attempt: u64,
    started: Instant,
    enabled: bool,
}
impl Initialization {
    pub(super) fn new() -> Self {
        Self {
            attempt: NEXT_ATTEMPT.fetch_add(1, Ordering::Relaxed),
            started: Instant::now(),
            enabled: cfg!(debug_assertions)
                && std::env::var_os("LAPIS_APP_BACKUP_DIAGNOSTICS")
                    .is_some_and(|value| value == "1"),
        }
    }
    pub(super) fn service<T>(&self, stage: Stage, result: Result<T>) -> Result<T> {
        self.emit(stage, result.is_ok(), "service", None);
        result
    }
    pub(super) fn io<T>(&self, stage: Stage, result: io::Result<T>) -> Result<T> {
        let os_code = result.as_ref().err().and_then(io::Error::raw_os_error);
        self.emit(stage, result.is_ok(), "io", os_code);
        result.map_err(|_| failure())
    }
    pub(super) fn native<T, E>(
        &self,
        stage: Stage,
        result: std::result::Result<T, E>,
    ) -> Result<T> {
        self.emit(stage, result.is_ok(), "native", None);
        result.map_err(|_| failure())
    }
    pub(super) fn watcher<T>(&self, stage: Stage, result: notify::Result<T>) -> Result<T> {
        let os_code = result.as_ref().err().and_then(|error| match &error.kind {
            notify::ErrorKind::Io(error) => error.raw_os_error(),
            _ => None,
        });
        self.emit(stage, result.is_ok(), "watcher", os_code);
        result.map_err(|_| failure())
    }
    fn emit(&self, stage: Stage, success: bool, category: &str, os_code: Option<i32>) {
        if self.enabled {
            // A diagnostic sink failure must not change accepted backup work.
            let _ = writeln!(
                io::stderr().lock(),
                "{}",
                self.line(stage, success, category, os_code)
            );
        }
    }
    fn line(&self, stage: Stage, success: bool, category: &str, os_code: Option<i32>) -> String {
        format!(
            "lapis-backup-init pid={} attempt={} state=initializing stage={stage:?} outcome={} category={category} os_code={} elapsed_ms={}",
            std::process::id(),
            self.attempt,
            if success { "ok" } else { "failed" },
            os_code.map_or_else(|| "none".into(), |code| code.to_string()),
            self.started.elapsed().as_millis(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::Error;

    fn diagnostics() -> Initialization {
        Initialization {
            attempt: 7,
            started: Instant::now(),
            enabled: false,
        }
    }
    #[test]
    fn io_failure_keeps_only_safe_stage_and_os_code() {
        let diagnostic = diagnostics();
        let error = io::Error::from_raw_os_error(13);
        let line = diagnostic.line(Stage::Executor, false, "io", error.raw_os_error());
        assert!(line.contains(
            "attempt=7 state=initializing stage=Executor outcome=failed category=io os_code=13"
        ));
        assert!(line.contains("elapsed_ms="));
        let failure = diagnostic
            .io::<()>(Stage::Executor, Err(error))
            .unwrap_err();
        assert_eq!(failure.code(), lapis_app_protocol::ErrorCode::Io);
        assert_eq!(failure.to_string(), "Automatic vault backup is unavailable");
    }
    #[test]
    fn native_and_watcher_context_never_reaches_the_public_error() {
        let diagnostic = diagnostics();
        let secret = "/private/user-vault/secret-token";
        let native = diagnostic
            .native::<(), _>(Stage::InitialCapture, Err(secret))
            .unwrap_err();
        let watcher = diagnostic
            .watcher::<()>(Stage::WatchRoot, Err(notify::Error::generic(secret)))
            .unwrap_err();
        for error in [native, watcher] {
            assert_eq!(error.to_string(), "Automatic vault backup is unavailable");
            assert!(!error.to_string().contains(secret));
        }
        let line = diagnostic.line(Stage::WatchRoot, false, "watcher", None);
        assert!(!line.contains(secret));
        assert!(line.contains("stage=WatchRoot outcome=failed category=watcher os_code=none"));
    }
    #[test]
    fn diagnostics_preserve_success_values_and_existing_service_errors() {
        let diagnostic = diagnostics();
        assert_eq!(
            diagnostic
                .native::<_, ()>(Stage::StartWorker, Ok(42))
                .unwrap(),
            42
        );
        let error = Error::new(
            lapis_app_protocol::ErrorCode::Closed,
            "Vault service is closed",
        );
        let error = diagnostic
            .service::<()>(Stage::Admission, Err(error))
            .unwrap_err();
        assert_eq!(error.code(), lapis_app_protocol::ErrorCode::Closed);
        assert_eq!(error.to_string(), "Vault service is closed");
    }
}
