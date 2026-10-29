use super::{Error, Result, guards, state::State};
use crate::vault::WriteOptions;
use lapis_app_protocol::{ErrorCode, ExpectedVersion};
use sha2::{Digest, Sha256};
use std::io::{self, Read};
use uuid::Uuid;

pub(super) struct Hashed<R> {
    source: R,
    hash: Sha256,
}
impl<R> Hashed<R> {
    pub fn new(source: R) -> Self {
        Self {
            source,
            hash: Sha256::new(),
        }
    }
    pub fn version(self) -> String {
        format!("{:x}", self.hash.finalize())
    }
}
impl<R: Read> Read for Hashed<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.source.read(buffer)?;
        self.hash.update(&buffer[..count]);
        Ok(count)
    }
}

pub(super) fn write(
    state: &State,
    path: &str,
    source: impl Read,
    expected: Option<&ExpectedVersion>,
    options: &WriteOptions,
    writer: Option<Uuid>,
) -> Result<String> {
    state.root.verify_physical_root()?;
    guards::write(state, path, writer, false)?;
    let mut source = Hashed::new(source);
    match expected {
        None => state.root.write_from(path, &mut source, options)?,
        Some(ExpectedVersion::Missing {}) => state
            .root
            .create_file(path, &mut source, options)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    Error::new(ErrorCode::Conflict, "File already exists")
                } else {
                    error.into()
                }
            })?,
        Some(ExpectedVersion::Version { version }) => {
            let bytes = read_versioned(state, path, version)?;
            state
                .root
                .write_if_unchanged(path, &mut source, Some(&bytes), options)?;
        }
    }
    Ok(source.version())
}

pub(super) fn read_versioned(state: &State, path: &str, version: &str) -> Result<Vec<u8>> {
    if version.len() != 64
        || !version
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::new(
            ErrorCode::InvalidRequest,
            "Invalid content version",
        ));
    }
    let bytes = state.root.read(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            Error::new(ErrorCode::Conflict, "File was removed")
        } else {
            error.into()
        }
    })?;
    if format!("{:x}", Sha256::digest(&bytes)) != version {
        return Err(Error::new(
            ErrorCode::Conflict,
            "File changed since it was read",
        ));
    }
    Ok(bytes)
}
