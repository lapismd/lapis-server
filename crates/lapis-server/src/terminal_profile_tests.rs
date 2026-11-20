use super::*;
use lapis_app_core::services::{Client, Runtime};
use lapis_app_protocol::terminal::{TerminalCommand as C, TerminalValue};
use std::{
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{Duration, Instant},
};

const ID: &str = "92b3bbf0-07f6-468e-a424-06d7bbdba797";
struct Terminal {
    _home: tempfile::TempDir,
    root: PathBuf,
    runtime: Runtime,
    client: Client,
    session: Uuid,
    cursor: u64,
}
impl Terminal {
    fn new(profile: bool, startup: Option<&str>, cwd: Option<&str>) -> Self {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("vaults").join(ID).join("content");
        std::fs::create_dir_all(&root).unwrap();
        if let Some(cwd) = cwd {
            std::fs::create_dir_all(root.join(cwd)).unwrap();
        }
        if let Some(startup) = startup {
            std::fs::write(home.path().join(".bash_profile"), ". \"$HOME/.bashrc\"\n").unwrap();
            std::fs::write(home.path().join(".bashrc"), startup).unwrap();
        }
        let path = home.path().to_owned();
        let runtime = Runtime::default().with_terminal_shell(move |root, id| {
            let mut launch = command(root, id, profile);
            if let Some(program) = std::env::var_os("LAPIS_TEST_BASH") {
                launch.program = program.into();
            }
            launch
                .env
                .push(("HOME".into(), path.as_os_str().to_owned()));
            launch.env.push(("HISTFILE".into(), "/dev/null".into()));
            launch
        });
        let client = runtime
            .admit(Uuid::parse_str(ID).unwrap(), &root)
            .unwrap()
            .attach()
            .unwrap();
        let TerminalValue::Created { session_id } = client
            .terminal(C::Create {
                cwd: cwd.map(Into::into),
                cols: 240,
                rows: 40,
            })
            .unwrap()
        else {
            panic!("missing terminal")
        };
        Self {
            _home: home,
            root,
            runtime,
            client,
            session: session_id,
            cursor: 0,
        }
    }
    fn read_until(&mut self, needle: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut result = String::new();
        loop {
            let output = self
                .client
                .terminal_output(self.session, self.cursor, Duration::from_millis(100))
                .unwrap();
            self.cursor = output.cursor;
            result.push_str(&String::from_utf8_lossy(&output.bytes));
            if result.contains(needle) {
                return result;
            }
            assert!(
                Instant::now() < deadline,
                "missing {needle:?} in {result:?}"
            );
        }
    }
    fn write(&self, line: &str) {
        self.client
            .terminal(C::Write {
                session_id: self.session,
                data: format!("{line}\n"),
            })
            .unwrap();
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        self.runtime.shutdown().unwrap();
    }
}

#[test]
fn prompt_colours_and_uuid_are_display_only_and_follow_command_status() {
    let mut terminal = Terminal::new(true, None, None);
    let first = terminal.read_until("\x1b[32m❯\x1b[0m");
    assert!(
        first.contains("\x1b[36m~/vaults/\x1b[35m92b3bbf0\x1b[36m/content"),
        "{first:?}"
    );
    assert!(
        !first.contains(ID),
        "full UUID must not appear in the prompt: {first:?}"
    );
    terminal.write("false");
    terminal.read_until("\x1b[31m❯\x1b[0m");
    terminal.write("printf 'EXIT_STATUS=%s\\n' \"$?\"");
    let status = terminal.read_until("\x1b[32m❯\x1b[0m");
    assert!(status.contains("EXIT_STATUS=1\r\n"), "{status:?}");
    terminal.write("pwd");
    let pwd = terminal.read_until("\x1b[32m❯\x1b[0m");
    assert!(pwd.contains(terminal.root.to_str().unwrap()), "{pwd:?}");
    assert!(pwd.contains(ID));
}

#[test]
fn startup_files_and_prompt_hooks_are_preserved_without_rewriting_them() {
    let startup = "printf 'STARTUP_ONCE\\n'\nPROMPT_COMMAND='user_status=$?; printf \"USER_HOOK=%s\\n\" \"$user_status\"'\n";
    let mut terminal = Terminal::new(true, Some(startup), None);
    let first = terminal.read_until("\x1b[32m❯\x1b[0m");
    assert_eq!(first.matches("STARTUP_ONCE").count(), 1, "{first:?}");
    terminal.write("false");
    let failed = terminal.read_until("\x1b[31m❯\x1b[0m");
    assert!(failed.contains("USER_HOOK=1\r\n"), "{failed:?}");
    assert_eq!(
        std::fs::read_to_string(terminal._home.path().join(".bashrc")).unwrap(),
        startup
    );
    terminal.write("shopt -q login_shell && [[ $- == *i* ]] && printf 'LOGIN_INTERACTIVE_OK\\n'");
    assert!(
        terminal
            .read_until("\x1b[32m❯\x1b[0m")
            .contains("LOGIN_INTERACTIVE_OK\r\n")
    );
}

#[test]
fn child_shells_do_not_inherit_private_prompt_hooks() {
    let mut terminal = Terminal::new(true, None, None);
    terminal.read_until("\x1b[32m❯\x1b[0m");
    terminal.write("bash --noprofile --norc -i");
    terminal.write("printf 'NESTED_%s\\n' OK");
    let nested = terminal.read_until("NESTED_OK\r\n");
    assert!(!nested.contains("command not found"), "{nested:?}");
    terminal.write("exit");
    terminal.read_until("\x1b[32m❯\x1b[0m");
}

#[test]
fn array_prompt_hooks_keep_their_native_order() {
    let startup = "if (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )); then\nPROMPT_COMMAND=('printf FIRST_HOOK' 'printf SECOND_HOOK')\nprintf 'ARRAY_SUPPORTED\\n'\nelse\nPROMPT_COMMAND='printf FIRST_HOOK'\nfi\n";
    let mut terminal = Terminal::new(true, Some(startup), None);
    let first = terminal.read_until("\x1b[32m❯\x1b[0m");
    assert!(first.contains("FIRST_HOOK"), "{first:?}");
    if first.contains("ARRAY_SUPPORTED\r\n") {
        assert!(first.contains("FIRST_HOOKSECOND_HOOK"), "{first:?}");
    }
}

#[test]
fn coloured_ls_is_real_shell_output() {
    let mut terminal = Terminal::new(true, None, None);
    std::fs::create_dir(terminal.root.join("notes")).unwrap();
    std::fs::write(terminal.root.join("backup.sh"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(
        terminal.root.join("backup.sh"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    terminal.read_until("\x1b[32m❯\x1b[0m");
    terminal.write("printf '__LIST_START__\\n'; ls; printf '__LIST_END__\\n'");
    let output = terminal.read_until("\x1b[32m❯\x1b[0m");
    let listing = output
        .split("__LIST_START__\r\n")
        .nth(1)
        .unwrap()
        .split("__LIST_END__\r\n")
        .next()
        .unwrap();
    assert!(
        listing.contains("notes") && listing.contains("backup.sh"),
        "{output:?}"
    );
    assert!(
        listing.contains("\x1b["),
        "ls output is monochrome: {output:?}"
    );
    for name in ["notes", "backup.sh"] {
        let preceding = &listing[..listing.find(name).unwrap()];
        let (_, style) = preceding.rsplit_once("\x1b[").expect("missing ls style");
        let (codes, _) = style.split_once('m').expect("missing SGR terminator");
        assert!(
            codes.split(';').any(|code| code
                .parse::<u16>()
                .is_ok_and(|value| (30..=38).contains(&value) || (90..=97).contains(&value))),
            "{name} has no foreground colour: {listing:?}"
        );
    }
}

#[test]
fn cwd_metacharacters_remain_data_and_foreign_uuids_are_not_shortened() {
    let foreign = "12345678-1234-1234-1234-123456789abc";
    let cwd = format!(
        "notes/$(touch PROFILE_INJECTION)`touch BACKTICK_INJECTION`/vaults/{foreign}/content"
    );
    let mut terminal = Terminal::new(true, None, Some(&cwd));
    let prompt = terminal.read_until("\x1b[32m❯\x1b[0m");
    assert!(prompt.contains(foreign), "{prompt:?}");
    assert!(prompt.contains("$(touch PROFILE_INJECTION)"), "{prompt:?}");
    assert!(!terminal.root.join(&cwd).join("PROFILE_INJECTION").exists());
    assert!(!terminal.root.join(&cwd).join("BACKTICK_INJECTION").exists());
    terminal.write("pwd");
    let output = terminal.read_until("\x1b[32m❯\x1b[0m");
    assert!(
        output.contains(terminal.root.join(&cwd).to_str().unwrap()),
        "{output:?}"
    );
}

#[test]
fn opt_out_keeps_the_original_login_shell_and_full_paths() {
    assert!(enabled(None));
    assert!(!enabled(Some(OsStr::new("0"))));
    let launch = command(Path::new("/tmp"), Uuid::nil(), false);
    assert_eq!(launch.args, vec![std::ffi::OsString::from("-il")]);
    assert!(!launch.env.iter().any(|(name, _)| name == "PROMPT_COMMAND"));
    let mut terminal = Terminal::new(false, None, None);
    terminal
        .write("shopt -q login_shell && [[ $- == *i* ]] && printf 'UNMANAGED_LOGIN_OK\\n'; pwd");
    let output = terminal.read_until(&format!("{}\r\n", terminal.root.display()));
    assert!(output.contains("UNMANAGED_LOGIN_OK\r\n"), "{output:?}");
    assert!(!output.contains("\x1b[35m92b3bbf0\x1b[36m"), "{output:?}");
}
