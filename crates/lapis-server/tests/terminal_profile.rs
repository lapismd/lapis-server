use lapis_app_protocol::terminal::{TerminalCommand, TerminalValue};
use lapis_server::{
    config::{Config, VaultConfig},
    host::{Host, Principal},
};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[test]
fn server_defaults_apply_the_reviewed_prompt_to_the_real_native_shell() {
    if std::env::var_os("LAPIS_TERMINAL_PROFILE_TEST_CHILD").is_none() {
        let home = tempfile::tempdir().unwrap();
        for startup in ["configured", "profile"] {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "server_defaults_apply_the_reviewed_prompt_to_the_real_native_shell",
                    "--nocapture",
                ])
                .env("LAPIS_TERMINAL_PROFILE_TEST_CHILD", startup)
                .env_remove("LAPIS_SERVER_TERMINAL_PROFILE")
                .env("HOME", home.path())
                .env("HISTFILE", "/dev/null")
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{startup}: {}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
        }
        return;
    }
    let storage = tempfile::tempdir().unwrap();
    let vault_id = Uuid::parse_str("92b3bbf0-07f6-468e-a424-06d7bbdba797").unwrap();
    let root = storage
        .path()
        .join("vaults")
        .join(vault_id.to_string())
        .join("content");
    std::fs::create_dir_all(&root).unwrap();
    let config = Config {
        vaults: vec![VaultConfig {
            id: vault_id,
            name: "Profile test".into(),
            root,
        }],
    };
    let profile = tempfile::tempdir().unwrap();
    let host = if std::env::var("LAPIS_TERMINAL_PROFILE_TEST_CHILD").unwrap() == "profile" {
        Host::with_profile(config, profile.path())
    } else {
        Host::new(config)
    }
    .unwrap();
    let client = host.attach(vault_id, Principal::Token).unwrap().client_id;
    let TerminalValue::Created { session_id } = host
        .terminal(
            client,
            Principal::Token,
            TerminalCommand::Create {
                cwd: None,
                cols: 160,
                rows: 24,
            },
        )
        .unwrap()
    else {
        panic!("missing shell")
    };
    let mut cursor = 0;
    let mut output = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let frame = host
            .terminal_output(client, Principal::Token, session_id, cursor)
            .unwrap();
        cursor = frame.cursor;
        output.extend(frame.bytes);
        if String::from_utf8_lossy(&output).contains("\x1b[35m92b3bbf0\x1b[36m") {
            break;
        }
        if Instant::now() >= deadline {
            host.shutdown().unwrap();
            panic!(
                "managed native prompt missing: {}",
                String::from_utf8_lossy(&output)
            );
        }
    }
    host.shutdown().unwrap();
}
