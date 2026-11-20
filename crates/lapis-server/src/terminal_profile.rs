//! Server-owned Bash presentation; native PTY and other hosts retain their defaults.
use lapis_app_core::services::ShellCommand;
use std::{ffi::OsStr, path::Path};
use uuid::Uuid;

pub(crate) fn enabled(value: Option<&OsStr>) -> bool {
    value != Some(OsStr::new("0"))
}

pub(crate) fn command(root: &Path, vault: Uuid, profile: bool) -> ShellCommand {
    let mut shell = ShellCommand::login_interactive(Path::new("/bin/bash"));
    shell.env.push(("SHELL".into(), "/bin/bash".into()));
    if profile {
        // Initialize before the first prompt, without writing a startup file or
        // sending script text through the user's PTY input stream. The script
        // loads the normal login files and composes their prompt hooks itself.
        shell.args = ["--noprofile", "--norc", "--login", "-i"]
            .map(Into::into)
            .into();
        shell.env.extend([
            (
                "PROMPT_COMMAND".into(),
                include_str!("terminal_profile.bash").into(),
            ),
            (
                "LAPIS_SERVER_TERMINAL_ROOT".into(),
                root.as_os_str().to_owned(),
            ),
            (
                "LAPIS_SERVER_TERMINAL_VAULT_ID".into(),
                vault.to_string().into(),
            ),
            (
                "LAPIS_SERVER_INHERITED_PROMPT_COMMAND".into(),
                std::env::var_os("PROMPT_COMMAND").unwrap_or_default(),
            ),
        ]);
    }
    shell
}

#[cfg(test)]
#[path = "terminal_profile_tests.rs"]
mod tests;
