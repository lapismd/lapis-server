use lapis_server::credentials::{CredentialKey, KEY_ENV, KEY_FILE_ENV};
use std::{
    env,
    io::{self, Write as _},
    os::{
        fd::OwnedFd,
        unix::{net::UnixStream, process::CommandExt as _},
    },
    process::{Command, Stdio},
};
use zeroize::Zeroizing;

const KEY_STDIN: &str = "--credential-key-stdin";

pub fn receive(
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
) -> io::Result<(bool, Option<CredentialKey>)> {
    let key = env::var_os(KEY_ENV);
    let file = env::var_os(KEY_FILE_ENV);
    if key.is_some() && file.is_some() {
        return Err(io::Error::other(format!(
            "Set only one of {KEY_ENV} or {KEY_FILE_ENV}"
        )));
    }
    if args.peek().is_some_and(|value| value == KEY_STDIN) {
        args.next();
        if key.is_some() || file.is_some() {
            return Err(io::Error::other(
                "Conflicting credential key startup inputs",
            ));
        }
        return Ok((true, CredentialKey::read(io::stdin().lock())));
    }
    if let Some(key) = key {
        let key = Zeroizing::new(key.into_encoded_bytes());
        return reexec(CredentialKey::from_base64(&key));
    }
    Ok((false, None))
}

fn reexec<T>(key: Option<CredentialKey>) -> io::Result<T> {
    // Exec replaces the original process environment, including the startup
    // environment visible through procfs. Children inherit the clean environment.
    let (reader, mut writer) = UnixStream::pair()?;
    if let Some(key) = key {
        writer.write_all(key.as_bytes())?;
    }
    drop(writer);
    Err(Command::new(env::current_exe()?)
        .arg(KEY_STDIN)
        .args(env::args_os().skip(1))
        .env_remove(KEY_ENV)
        .stdin(Stdio::from(OwnedFd::from(reader)))
        .exec())
}
