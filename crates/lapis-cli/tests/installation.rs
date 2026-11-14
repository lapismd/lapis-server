#![cfg(unix)]
use std::{fs, process::Command};
use tempfile::TempDir;

fn cli(home: &TempDir, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lapis"))
        .env("HOME", home.path())
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn installs_and_removes_command_without_a_server_or_credential() {
    let home = TempDir::new().unwrap();
    let before = cli(&home, &["cli", "status"]);
    assert!(
        before.status.success(),
        "{}",
        String::from_utf8_lossy(&before.stderr)
    );
    assert!(String::from_utf8_lossy(&before.stdout).contains("Not installed"));
    for _ in 0..2 {
        let result = cli(&home, &["cli", "install"]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let installed = home.path().join(".local/bin/lapis");
    assert!(
        Command::new(&installed)
            .arg("--version")
            .status()
            .unwrap()
            .success()
    );
    assert!(cli(&home, &["cli", "uninstall"]).status.success());
    assert!(!installed.exists());
    assert!(cli(&home, &["cli", "uninstall"]).status.success());
}
#[test]
fn refuses_foreign_commands_and_preserves_shell_configuration() {
    let home = TempDir::new().unwrap();
    fs::create_dir_all(home.path().join(".local/bin")).unwrap();
    let path = home.path().join(".local/bin/lapis");
    fs::write(&path, "foreign command").unwrap();
    assert!(!cli(&home, &["cli", "install"]).status.success());
    assert!(!cli(&home, &["cli", "uninstall"]).status.success());
    assert_eq!(fs::read_to_string(path).unwrap(), "foreign command");
}
#[test]
fn shell_setup_is_opt_in_idempotent_and_removes_only_unchanged_blocks() {
    let home = TempDir::new().unwrap();
    fs::write(home.path().join(".zshrc"), "# user configuration\n").unwrap();
    assert!(cli(&home, &["cli", "install"]).status.success());
    assert_eq!(
        fs::read_to_string(home.path().join(".zshrc")).unwrap(),
        "# user configuration\n"
    );
    for _ in 0..2 {
        assert!(
            cli(&home, &["cli", "install", "--setup-path", "zsh"])
                .status
                .success()
        );
    }
    let text = fs::read_to_string(home.path().join(".zshrc")).unwrap();
    assert_eq!(text.matches("# >>> Lapis CLI PATH >>>").count(), 1);
    assert!(
        cli(&home, &["cli", "uninstall", "--remove-path"])
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(home.path().join(".zshrc")).unwrap(),
        "# user configuration\n"
    );
    assert!(
        cli(&home, &["cli", "install", "--setup-path", "zsh"])
            .status
            .success()
    );
    let changed = fs::read_to_string(home.path().join(".zshrc"))
        .unwrap()
        .replace("export PATH=", "export USER_EDITED_PATH=");
    fs::write(home.path().join(".zshrc"), &changed).unwrap();
    assert!(
        cli(&home, &["cli", "uninstall", "--remove-path"])
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(home.path().join(".zshrc")).unwrap(),
        changed
    );
}
#[test]
fn refuses_foreign_and_dangling_links_and_symlink_parents() {
    use std::os::unix::fs::symlink;
    let home = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::create_dir_all(home.path().join(".local/bin")).unwrap();
    let path = home.path().join(".local/bin/lapis");
    symlink(outside.path().join("missing"), &path).unwrap();
    assert!(!cli(&home, &["cli", "install"]).status.success());
    assert!(!cli(&home, &["cli", "uninstall"]).status.success());
    assert!(fs::symlink_metadata(&path).unwrap().is_symlink());
    fs::remove_file(&path).unwrap();
    fs::remove_dir(home.path().join(".local/bin")).unwrap();
    symlink(outside.path(), home.path().join(".local/bin")).unwrap();
    assert!(!cli(&home, &["cli", "install"]).status.success());
    assert!(!outside.path().join("lapis").exists());
}
#[test]
fn repairs_missing_owned_binary_and_refuses_shell_symlinks() {
    use std::os::unix::fs::symlink;
    let home = TempDir::new().unwrap();
    assert!(cli(&home, &["cli", "install"]).status.success());
    fs::remove_file(home.path().join(".local/share/lapis/cli/lapis")).unwrap();
    assert!(
        String::from_utf8_lossy(&cli(&home, &["cli", "status"]).stdout).contains("Needs repair")
    );
    assert!(cli(&home, &["cli", "install"]).status.success());
    let outside = TempDir::new().unwrap();
    let file = outside.path().join("shell");
    fs::write(&file, "untouched").unwrap();
    symlink(&file, home.path().join(".zshrc")).unwrap();
    assert!(
        !cli(&home, &["cli", "install", "--setup-path", "zsh"])
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(file).unwrap(), "untouched");
}
#[test]
fn json_status_preserves_paths_with_spaces_quotes_and_newlines() {
    let parent = TempDir::new().unwrap();
    let path = parent.path().join("home 'quoted'\nline");
    fs::create_dir(&path).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_lapis"))
        .env("HOME", &path)
        .args(["cli", "status", "--json"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["state"], "not-installed");
    assert_eq!(
        value["destination"],
        path.join(".local/bin/lapis").to_str().unwrap()
    );
}
#[test]
fn receipts_repair_missing_links_refuse_tampering_and_require_explicit_source_switch() {
    use std::os::unix::fs::PermissionsExt;
    let home = TempDir::new().unwrap();
    assert!(cli(&home, &["cli", "install"]).status.success());
    fs::remove_file(home.path().join(".local/bin/lapis")).unwrap();
    assert!(cli(&home, &["cli", "install"]).status.success());
    let other = home.path().join("another-source");
    fs::copy(env!("CARGO_BIN_EXE_lapis"), &other).unwrap();
    fs::set_permissions(&other, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
        !Command::new(&other)
            .env("HOME", home.path())
            .args(["cli", "install"])
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(&other)
            .env("HOME", home.path())
            .args(["cli", "install", "--replace"])
            .status()
            .unwrap()
            .success()
    );
    fs::write(
        home.path().join(".local/share/lapis/cli/lapis"),
        "user replaced this binary",
    )
    .unwrap();
    assert!(!cli(&home, &["cli", "uninstall"]).status.success());
    assert!(
        !cli(&home, &["cli", "install", "--replace"])
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(home.path().join(".local/share/lapis/cli/lapis")).unwrap(),
        "user replaced this binary"
    );
}
#[test]
fn shell_setup_preserves_original_bytes_without_a_final_newline() {
    let home = TempDir::new().unwrap();
    let path = home.path().join(".zshrc");
    fs::write(&path, "export USER_CONFIGURATION=retained").unwrap();
    assert!(
        cli(&home, &["cli", "install", "--setup-path", "zsh"])
            .status
            .success()
    );
    let shell = Command::new("bash")
        .env("HOME", home.path())
        .args([
            "--noprofile",
            "--norc",
            "-c",
            ". \"$HOME/.zshrc\"; . \"$HOME/.zshrc\"; command -v lapis",
        ])
        .output()
        .unwrap();
    assert!(shell.status.success());
    assert_eq!(
        String::from_utf8(shell.stdout).unwrap().trim(),
        home.path().join(".local/bin/lapis").to_str().unwrap()
    );
    assert!(
        cli(&home, &["cli", "uninstall", "--remove-path"])
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "export USER_CONFIGURATION=retained"
    );
}
