use super::*;
use axum::{body::Body, extract::Request, response::Response};
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(unix)]
#[tokio::test]
async fn token_files_require_private_regular_owned_files_and_refuse_symlinks_and_fifos() {
    use std::os::unix::fs::{FileTypeExt, PermissionsExt, symlink};
    let f = Fixture::start().await;
    let credentials = tempfile::tempdir().unwrap();
    let path = credentials.path().join("token");
    fs::write(&path, &f.token).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    async fn file(base: &str, path: &std::path::Path) -> std::process::Output {
        tokio::time::timeout(
            Duration::from_secs(5),
            Command::new(env!("CARGO_BIN_EXE_lapis"))
                .args(["--target", base, "--token-file"])
                .arg(path)
                .args(["--json", "capabilities"])
                .stdin(Stdio::null())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .unwrap()
        .unwrap()
    }
    value(&file(&f.base, &path).await);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(file(&f.base, &path).await.status.code(), Some(3));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let alias = credentials.path().join("link");
    symlink(&path, &alias).unwrap();
    assert_eq!(file(&f.base, &alias).await.status.code(), Some(3));
    let fifo = credentials.path().join("fifo");
    // POSIX mkfifo is available on both macOS and Linux; rustix::mkfifoat
    // is intentionally not exported for Apple targets.
    assert!(
        std::process::Command::new("mkfifo")
            .args(["-m", "600"])
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
    assert_eq!(file(&f.base, &fifo).await.status.code(), Some(3));
}

#[tokio::test]
async fn stdin_credentials_are_bounded_and_never_echoed() {
    let f = Fixture::start().await;
    for token in [
        String::new(),
        "secret\nsecond-line".into(),
        "x".repeat(5000),
    ] {
        let output = run(&f.base, &token, &["capabilities"], true).await;
        assert_eq!(output.status.code(), Some(3));
        assert!(
            matches!(reply(&output).outcome, ContentOutcome::Error { error } if error.code == ContentErrorCode::Unauthenticated)
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("secret"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("secret"));
    }
}

#[derive(Clone, Debug)]
enum Mode {
    WrongApi,
    WrongId,
    WrongKind,
    WeakVersion,
    Oversized,
    EchoError,
    Redirect(String),
    Slow,
}
struct Stub {
    base: String,
    hits: Arc<AtomicUsize>,
    server: tokio::task::JoinHandle<()>,
}
impl Stub {
    async fn start(mode: Mode) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let observed = hits.clone();
        let app = axum::Router::new().fallback(move |request: Request| {
            let mode = mode.clone();
            observed.fetch_add(1, Ordering::SeqCst);
            async move {
                let id = request.headers()["x-lapis-request-id"].to_str().unwrap();
                let mut response = Response::builder()
                    .header("x-lapis-content-api", "1")
                    .header(
                        "x-lapis-request-id",
                        if matches!(mode, Mode::WrongId) {
                            "wrong"
                        } else {
                            id
                        },
                    );
                if let Mode::Redirect(target) = mode {
                    return response
                        .status(307)
                        .header("location", target)
                        .body(Body::empty())
                        .unwrap();
                }
                if matches!(mode, Mode::Slow) {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                if matches!(mode, Mode::WeakVersion | Mode::Oversized) {
                    let body = if matches!(mode, Mode::Oversized) {
                        "x".repeat(MAX_CONTENT_TEXT_BYTES + 1)
                    } else {
                        "note".into()
                    };
                    return response
                        .header("content-type", "text/plain; charset=utf-8")
                        .header(
                            "etag",
                            if matches!(mode, Mode::WeakVersion) {
                                "W/\"version\""
                            } else {
                                "\"version\""
                            },
                        )
                        .body(Body::from_stream(futures_util::stream::iter([Ok::<
                            _,
                            std::convert::Infallible,
                        >(
                            body
                        )])))
                        .unwrap();
                }
                let outcome = if matches!(mode, Mode::EchoError) {
                    response = response.status(401);
                    ContentOutcome::Error {
                        error: ContentError {
                            code: ContentErrorCode::Unauthenticated,
                            message: request.headers()["authorization"].to_str().unwrap().into(),
                            effect: ContentEffectStatus::NotStarted,
                        },
                    }
                } else {
                    ContentOutcome::Ok {
                        value: ContentValue::Vaults {
                            items: vec![],
                            next_cursor: None,
                        },
                    }
                };
                let reply = ContentReply {
                    api_version: if matches!(mode, Mode::WrongApi) { 2 } else { 1 },
                    request_id: Uuid::parse_str(id).unwrap(),
                    outcome,
                };
                response
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&reply).unwrap()))
                    .unwrap()
            }
        });
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { base, hits, server }
    }
}
impl Drop for Stub {
    fn drop(&mut self) {
        self.server.abort();
    }
}

#[tokio::test]
async fn malformed_remote_contracts_and_chunked_oversized_notes_are_refused() {
    let id = Uuid::new_v4().to_string();
    for (mode, expected, note) in [
        (Mode::WrongApi, 8, false),
        (Mode::WrongId, 8, false),
        (Mode::WrongKind, 8, false),
        (Mode::WeakVersion, 8, true),
        (Mode::Oversized, 9, true),
        (Mode::EchoError, 3, false),
    ] {
        let label = format!("{mode:?}");
        let stub = Stub::start(mode).await;
        let args: &[&str] = if note {
            &["note", "read", "--vault", &id, "--path", "note.md"]
        } else {
            &["capabilities"]
        };
        let output = run(&stub.base, "private-token-value", args, true).await;
        assert_eq!(output.status.code(), Some(expected), "{label}");
        assert!(!String::from_utf8_lossy(&output.stdout).contains("private-token-value"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private-token-value"));
        assert_eq!(stub.hits.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn redirects_never_forward_credentials_and_timeouts_do_not_retry() {
    let destination = Stub::start(Mode::WrongKind).await;
    let redirect = Stub::start(Mode::Redirect(destination.base.clone())).await;
    let output = run(
        &redirect.base,
        "private-token-value",
        &["capabilities"],
        true,
    )
    .await;
    assert_eq!(output.status.code(), Some(8));
    assert_eq!(destination.hits.load(Ordering::SeqCst), 0);
    assert_eq!(redirect.hits.load(Ordering::SeqCst), 1);
    let slow = Stub::start(Mode::Slow).await;
    let output = run(&slow.base, "private-token-value", &["capabilities"], true).await;
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(slow.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn closing_stdout_is_normal_completion_for_json_and_human_output() {
    let f = Fixture::start().await;
    fs::write(f.root.path().join("note.md"), "x".repeat(16 * 1024)).unwrap();
    let vault = f.vault.to_string();
    for json in [true, false] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lapis"));
        command.args([
            "--target",
            &f.base,
            "--token-stdin",
            "note",
            "read",
            "--vault",
            &vault,
            "--path",
            "note.md",
        ]);
        if json {
            command.arg("--json");
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        // The CLI cannot issue its HTTP request until stdin reaches EOF.
        drop(child.stdout.take().unwrap());
        child
            .stdin
            .take()
            .unwrap()
            .write_all(f.token.as_bytes())
            .await
            .unwrap();
        let output = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
            .await
            .unwrap()
            .unwrap();
        assert!(output.status.success(), "JSON mode: {json}");
        assert!(output.stderr.is_empty());
    }
}
