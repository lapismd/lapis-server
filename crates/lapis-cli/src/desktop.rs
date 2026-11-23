//! Resolve an explicitly approved Desktop credential; discovery grants no authority.
use crate::{
    args::{Cli, Command},
    client, credential,
    errors::failure,
};
use lapis_app_protocol::content::{ContentError, ContentErrorCode};
use serde::Deserialize;
use std::{io::Read, path::PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Connection {
    version: u32,
    origin: String,
    token_file: PathBuf,
}

pub(crate) fn resolve(args: &mut Cli) -> Result<(), ContentError> {
    // Offline help must remain usable without a running host or home directory.
    if matches!(args.command, Command::Help { .. })
        || matches!(&args.command, Command::Operation(words) if words.iter().any(|w| w == "--help" || w == "-h"))
    {
        return Ok(());
    }
    let desktop = args.target.as_deref() == Some("desktop")
        || args.target.is_none() && args.token_file.is_none() && !args.token_stdin;
    if !desktop {
        return Ok(());
    }
    let invalid = || failure(ContentErrorCode::Unauthenticated);
    if args.token_file.is_some() || args.token_stdin {
        return Err(invalid());
    }
    let home = std::env::var_os("HOME")
        .filter(|p| !p.is_empty())
        .ok_or_else(invalid)?;
    let path = PathBuf::from(home).join(".local/state/lapis/desktop.json");
    if !path.try_exists().map_err(|_| invalid())? {
        return Err(failure(ContentErrorCode::InvalidRequest));
    }
    let mut bytes = Vec::new();
    credential::open(&path)?
        .take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() > 16384 {
        return Err(invalid());
    }
    let connection: Connection = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    let url = client::target(Some(&connection.origin))?;
    // A discovery record may never send a local credential to a remote origin.
    if connection.version != 1
        || url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none()
        || !connection.token_file.is_absolute()
    {
        return Err(invalid());
    }
    args.target = Some(connection.origin);
    args.token_file = Some(connection.token_file);
    Ok(())
}
