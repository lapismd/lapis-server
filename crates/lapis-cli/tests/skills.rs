use lapis_app_core::{
    services::{
        Runtime,
        content::{ContentGrants, ContentVault},
        plugin_operations::OperationRegistry,
    },
    skills::provider::SkillsProvider,
};
use std::{process::Stdio, sync::Arc, time::Duration};
use tokio::{io::AsyncWriteExt, process::Command};
use uuid::Uuid;

#[tokio::test]
async fn cli_and_native_app_service_share_one_vault_and_preserve_edits() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let owner = runtime.admit(id, root.path()).unwrap();
    let mut registry = OperationRegistry::default();
    registry
        .register(Arc::new(SkillsProvider::default()))
        .unwrap();
    let grants = Arc::new(ContentGrants::with_operations(Arc::new(registry)));
    grants.set_enabled(true).unwrap();
    let scoped = grants
        .issue_named_operations(
            "skills",
            vec![ContentVault {
                name: "test".into(),
                client: owner.attach().unwrap(),
            }],
            Duration::from_secs(60),
            &["skills:read".into(), "skills:write".into()],
        )
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = lapis_content_http::router(grants, &base).unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    // The native App service installs first; a fresh CLI process observes its receipt.
    let client = owner.attach().unwrap();
    let initial = client
        .manage_packaged_skills(
            &[lapis_app_core::skills::provider::cli_skill()],
            "install",
            None,
        )
        .unwrap();
    assert_eq!(initial[0].status, "installed");
    for action in ["list", "install", "update"] {
        let result = run(&base, scoped.token.expose_secret(), id, action).await;
        assert_eq!(result[0]["status"], "unchanged");
    }
    let file = root.path().join(".agents/skills/lapis-cli/SKILL.md");
    std::fs::write(&file, "user instructions").unwrap();
    let result = run(&base, scoped.token.expose_secret(), id, "update").await;
    assert_eq!(result[0]["reason"], "locally-modified");
    assert_eq!(std::fs::read_to_string(file).unwrap(), "user instructions");
    server.abort();
}
async fn run(base: &str, token: &str, vault: Uuid, action: &str) -> serde_json::Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lapis"))
        .args([
            "--target",
            base,
            "--token-stdin",
            "--json",
            "skills",
            action,
            "--vault",
            &vault.to_string(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(token.as_bytes())
        .await
        .unwrap();
    let output = child.wait_with_output().await.unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    value["outcome"]["value"].clone()
}
