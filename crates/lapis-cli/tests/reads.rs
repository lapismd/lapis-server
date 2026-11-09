use lapis_app_core::services::{
    Runtime,
    content::{ContentGrants, ContentVault},
};
use lapis_app_protocol::content::*;
use std::{fs, process::Stdio, sync::Arc, time::Duration};
use tokio::{io::AsyncWriteExt, process::Command};
use uuid::Uuid;

struct Fixture {
    root: tempfile::TempDir,
    vault: Uuid,
    grants: Arc<ContentGrants>,
    grant: Uuid,
    token: String,
    base: String,
    server: tokio::task::JoinHandle<()>,
    runtime: Runtime,
}
impl Fixture {
    async fn start() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("note.md"),
            "# Exact 🪨\r\nno final newline",
        )
        .unwrap();
        fs::write(root.path().join("second.md"), "second").unwrap();
        let runtime = Runtime::default();
        let vault = Uuid::new_v4();
        let native = runtime.admit(vault, root.path()).unwrap();
        let grants = Arc::new(ContentGrants::default());
        let grant = grants
            .issue_read_only(
                vec![ContentVault {
                    name: "Personal".into(),
                    client: native.attach().unwrap(),
                }],
                Duration::from_secs(60),
            )
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = lapis_content_http::router(grants.clone(), &base).unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            root,
            vault,
            grants,
            grant: grant.grant_id,
            token: grant.token.expose_secret().into(),
            base,
            server,
            runtime,
        }
    }
    async fn run(&self, args: &[&str], json: bool) -> std::process::Output {
        run(&self.base, &self.token, args, json).await
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
        self.grants.shutdown().unwrap();
    }
}
async fn run(base: &str, token: &str, args: &[&str], json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lapis"));
    command.args(["--target", base, "--token-stdin", "--timeout-ms", "1000"]);
    if json {
        command.arg("--json");
    }
    let mut child = command
        .args(args)
        .env("HTTP_PROXY", "http://127.0.0.1:1")
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    if let Err(error) = child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("{token}\n").as_bytes())
        .await
    {
        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
    }
    tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
        .await
        .unwrap()
        .unwrap()
}
fn reply(output: &std::process::Output) -> ContentReply {
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("invalid JSON output; exit {:?}", output.status.code()))
}
fn value(output: &std::process::Output) -> ContentValue {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reply = reply(output);
    assert_eq!(reply.api_version, 1);
    assert!(!reply.request_id.is_nil());
    match reply.outcome {
        ContentOutcome::Ok { value } => value,
        ContentOutcome::Error { error } => panic!("{error:?}"),
    }
}

#[tokio::test]
async fn binary_reads_the_shared_http_contract_without_opening_the_vault() {
    let f = Fixture::start().await;
    let ContentValue::Capabilities { capabilities } = value(&f.run(&["capabilities"], true).await)
    else {
        panic!()
    };
    assert_eq!(capabilities.operations.len(), 4);
    let ContentValue::Vaults { items, .. } = value(&f.run(&["vault", "list"], true).await) else {
        panic!()
    };
    assert_eq!(items[0].vault_id, f.vault);
    let id = f.vault.to_string();
    let ContentValue::Notes {
        items,
        next_cursor: Some(cursor),
    } = value(
        &f.run(&["note", "list", "--vault", &id, "--limit", "1"], true)
            .await,
    )
    else {
        panic!()
    };
    assert_eq!(items[0].path, "note.md");
    let ContentValue::Notes {
        items,
        next_cursor: None,
    } = value(
        &f.run(
            &[
                "note", "list", "--vault", &id, "--limit", "1", "--cursor", &cursor,
            ],
            true,
        )
        .await,
    )
    else {
        panic!()
    };
    assert_eq!(items[0].path, "second.md");
    let args = ["note", "read", "--vault", &id, "--path", "note.md"];
    let ContentValue::Note {
        text,
        version,
        path,
    } = value(&f.run(&args, true).await)
    else {
        panic!()
    };
    let saved = f
        .runtime
        .get(f.vault)
        .unwrap()
        .attach()
        .unwrap()
        .read_text("note.md")
        .unwrap();
    assert_eq!(
        (path.as_str(), text.as_str(), version.as_str()),
        ("note.md", saved.text.as_str(), saved.version.as_str())
    );
    let raw = f.run(&args, false).await;
    assert!(raw.status.success());
    assert_eq!(raw.stdout, fs::read(f.root.path().join("note.md")).unwrap());
    assert!(raw.stderr.is_empty());
}

#[tokio::test]
async fn binary_reports_typed_errors_with_stable_exit_codes_and_clean_streams() {
    let f = Fixture::start().await;
    let foreign = Uuid::new_v4().to_string();
    let output = f
        .run(
            &["note", "read", "--vault", &foreign, "--path", "note.md"],
            true,
        )
        .await;
    assert_eq!(output.status.code(), Some(6));
    assert!(
        matches!(reply(&output).outcome, ContentOutcome::Error { error } if error.code == ContentErrorCode::PermissionDenied)
    );
    let output = f
        .run(
            &[
                "note",
                "read",
                "--vault",
                &f.vault.to_string(),
                "--path",
                "missing.md",
            ],
            false,
        )
        .await;
    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
    f.grants.revoke(f.grant).unwrap();
    let output = f.run(&["capabilities"], true).await;
    assert_eq!(output.status.code(), Some(3));
    assert!(
        matches!(reply(&output).outcome, ContentOutcome::Error { error } if error.code == ContentErrorCode::Unauthenticated)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains(&f.token));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(&f.token));
}

#[tokio::test]
async fn help_and_invalid_targets_never_open_a_local_vault() {
    let output = Command::new(env!("CARGO_BIN_EXE_lapis"))
        .arg("--help")
        .output()
        .await
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("note"));
    for target in [
        "http://remote.example",
        "https://user:password@example.com",
        "https://example.com/path",
        "https://example.com?token=secret",
    ] {
        let output = run(target, "credential", &["capabilities"], true).await;
        assert_eq!(output.status.code(), Some(2));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("password"));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_lapis"))
        .args(["--json", "capabilities"])
        .output()
        .await
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[path = "reads/security.rs"]
mod security;
