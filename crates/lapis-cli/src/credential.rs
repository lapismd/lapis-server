use crate::{args::Cli, errors::failure};
use lapis_app_protocol::content::{ContentError, ContentErrorCode};
use std::{io::Read, path::Path};

pub(crate) fn read(args: &Cli) -> Result<String, ContentError> {
    let reader: Box<dyn Read> = match (&args.token_file, args.token_stdin) {
        (Some(path), false) => Box::new(open(path)?),
        (None, true) => Box::new(std::io::stdin()),
        _ => return Err(failure(ContentErrorCode::InvalidRequest)),
    };
    let mut bytes = Vec::new();
    reader
        .take(4099)
        .read_to_end(&mut bytes)
        .map_err(|_| failure(ContentErrorCode::Unavailable))?;
    if bytes.ends_with(b"\n") {
        bytes.pop();
        if bytes.ends_with(b"\r") {
            bytes.pop();
        }
    }
    if bytes.is_empty() || bytes.len() > 4096 || !bytes.iter().all(u8::is_ascii_graphic) {
        return Err(failure(ContentErrorCode::Unauthenticated));
    }
    String::from_utf8(bytes).map_err(|_| failure(ContentErrorCode::Unauthenticated))
}

#[cfg(unix)]
fn open(path: &Path) -> Result<std::fs::File, ContentError> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags.bits() as i32)
        .open(path)
        .map_err(|_| failure(ContentErrorCode::Unauthenticated))?;
    let metadata = file
        .metadata()
        .map_err(|_| failure(ContentErrorCode::Unauthenticated))?;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(failure(ContentErrorCode::Unauthenticated));
    }
    Ok(file)
}
#[cfg(not(unix))]
fn open(_: &Path) -> Result<std::fs::File, ContentError> {
    Err(failure(ContentErrorCode::Unsupported))
}
