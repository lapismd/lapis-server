use std::process::Command;

#[test]
fn contributed_tasks_help_is_offline_and_documents_real_arguments() {
    for arguments in [
        vec!["tasks", "complete", "--help"],
        vec!["help", "tasks", "complete"],
        vec![
            "--target",
            "https://unavailable.invalid",
            "tasks",
            "complete",
            "--help",
        ],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_lapis"))
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let help = String::from_utf8(result.stdout).unwrap();
        assert!(help.contains("--if-version"));
        assert!(help.contains("--local-date"));
        assert!(help.contains("--input-file"));
        assert!(help.contains("--invocation-id"));
        assert!(help.contains("inspect"));
        assert!(help.contains("Usage: lapis tasks complete"));
    }
}

#[test]
fn keyword_help_and_all_seventeen_command_paths_are_available() {
    let result = Command::new(env!("CARGO_BIN_EXE_lapis"))
        .args(["help", "-k", "tasks"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let help = String::from_utf8(result.stdout).unwrap();
    for action in [
        "list",
        "get",
        "checklist",
        "plan",
        "start",
        "deadline",
        "inbox",
        "repeat",
        "tracking",
        "pause",
        "resume",
        "cancel",
        "reopen",
        "complete",
        "skip",
        "inspect",
        "retry",
    ] {
        assert!(
            help.contains(&format!("tasks {action}")),
            "missing {action}"
        );
    }
}

#[test]
fn packaged_skill_help_is_offline_and_describes_safe_updates() {
    for action in ["list", "install", "update"] {
        let result = Command::new(env!("CARGO_BIN_EXE_lapis"))
            .args(["help", "skills", action])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let help = String::from_utf8(result.stdout).unwrap();
        assert!(help.contains("--skill"));
        assert!(help.contains("skip edited"));
    }
}
