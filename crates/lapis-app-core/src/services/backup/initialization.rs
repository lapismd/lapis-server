//! Opt-in, bounded debug diagnostics. Never format native errors or paths.
use super::failure;
use crate::services::Result;
use std::{
    io::{self, Write},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

static NEXT_ATTEMPT: AtomicU64 = AtomicU64::new(1);
static NEXT_SHUTDOWN_ATTEMPT: AtomicU64 = AtomicU64::new(1);

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

#[derive(Clone, Copy, Debug)]
pub(super) enum ShutdownStage {
    ClientAdmission,
    WorkReservation,
    QueueReservation,
    SelectStorage,
    ValidateStorage,
    LockOwner,
    FinalCapture,
    ReleaseWatcher,
    QueueCompletion,
}

#[derive(Clone)]
pub(super) struct Shutdown {
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

impl Shutdown {
    pub(super) fn new() -> Self {
        Self {
            attempt: NEXT_SHUTDOWN_ATTEMPT.fetch_add(1, Ordering::Relaxed),
            started: Instant::now(),
            enabled: cfg!(debug_assertions)
                && std::env::var_os("LAPIS_APP_BACKUP_DIAGNOSTICS")
                    .is_some_and(|value| value == "1"),
        }
    }
    pub(super) fn service<T>(&self, stage: ShutdownStage, result: Result<T>) -> Result<T> {
        self.emit(stage, result.is_ok(), "service");
        result
    }
    pub(super) fn native<T>(
        &self,
        stage: ShutdownStage,
        result: std::result::Result<T, String>,
    ) -> Result<T> {
        let category = result
            .as_ref()
            .err()
            .map_or("native", |error| native_category(error));
        self.emit(stage, result.is_ok(), category);
        result.map_err(|_| failure())
    }
    fn emit(&self, stage: ShutdownStage, success: bool, category: &str) {
        if self.enabled {
            let _ = writeln!(
                io::stderr().lock(),
                "{}",
                self.line(stage, success, category)
            );
        }
    }
    fn line(&self, stage: ShutdownStage, success: bool, category: &str) -> String {
        format!(
            "lapis-backup-stop pid={} attempt={} state=stopping stage={stage:?} outcome={} category={category} elapsed_ms={}",
            std::process::id(),
            self.attempt,
            if success { "ok" } else { "failed" },
            self.started.elapsed().as_millis(),
        )
    }
}

fn native_category(error: &str) -> &'static str {
    if error == "Another vault backup command is running"
        || error == "Vault backup is busy or closed"
    {
        "native-busy"
    } else if error == "The vault backup binding is invalid or changed" {
        "native-binding"
    } else if error.starts_with("Vault backup IO:") {
        "native-io"
    } else if error.starts_with("Vault backup JJ:") {
        "native-jj"
    } else if matches!(
        error,
        "Vault backup stopped" | "Vault backup worker is closed" | "Vault backup worker failed"
    ) {
        "native-worker"
    } else {
        "native-other"
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
    fn shutdown_diagnostics() -> Shutdown {
        Shutdown {
            attempt: 9,
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
    #[test]
    fn shutdown_diagnostics_classify_without_retaining_native_error_text() {
        let diagnostics = shutdown_diagnostics();
        let secret = "/private/user-vault/secret-token";
        let line = diagnostics.line(
            ShutdownStage::FinalCapture,
            false,
            native_category(&format!("Vault backup JJ: {secret}")),
        );
        assert!(line.contains(
            "attempt=9 state=stopping stage=FinalCapture outcome=failed category=native-jj"
        ));
        assert!(line.contains("elapsed_ms="));
        assert!(!line.contains(secret));
        let error = diagnostics
            .native::<()>(
                ShutdownStage::FinalCapture,
                Err(format!("Vault backup IO: {secret}")),
            )
            .unwrap_err();
        assert_eq!(error.to_string(), "Automatic vault backup is unavailable");
        assert!(!error.to_string().contains(secret));
    }
}
