#![cfg(unix)]
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    process::Command,
};

#[test]
fn discovery_refuses_unsafe_records_and_keeps_mcp_stdout_clean() {
    let home = tempfile::tempdir().unwrap();
    let state = home.path().join(".local/state/lapis");
    fs::create_dir_all(&state).unwrap();
    let path = state.join("desktop.json");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_lapis"))
            .env("HOME", home.path())
            .args(["--target", "desktop", "mcp", "serve", "--stdio"])
            .output()
            .unwrap()
    };
    for body in [
        json!({"version":1,"origin":"https://remote.example","token_file":"/private/token"})
            .to_string(),
        json!({"version":2,"origin":"http://127.0.0.1:1234","token_file":"/private/token"})
            .to_string(),
        json!({"version":1,"origin":"http://127.0.0.1:1234","token_file":"relative"}).to_string(),
        "x".repeat(16385),
    ] {
        fs::write(&path, body).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let output = run();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("/private/token"));
    }
    fs::remove_file(&path).unwrap();
    let target = home.path().join("private");
    fs::write(&target, "{}").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&target, &path).unwrap();
    assert!(!run().status.success());
}
