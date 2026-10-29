use super::{Client, Error, Result, byte_writes, byte_writes::Hashed};
use crate::vault::{MAX_FILE_BYTES, WriteOptions};
use lapis_app_protocol::{ErrorCode, ExpectedVersion};
use std::io::{self, Read, Seek};

/// Native immutable representation, never a serialized file handle or pathname.
#[derive(Debug)]
pub struct FileSnapshot {
    pub file: std::fs::File,
    pub length: u64,
    pub version: String,
}
impl Client {
    pub fn snapshot(&self, path: &str) -> Result<FileSnapshot> {
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        state.root.verify_physical_root()?;
        let mut source = Hashed::new(state.root.open_file(path)?);
        let mut file = tempfile::tempfile()?;
        let length = io::copy(&mut (&mut source).take(MAX_FILE_BYTES + 1), &mut file)?;
        if length > MAX_FILE_BYTES {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "File exceeds the 64 MiB limit",
            ));
        }
        file.rewind()?;
        Ok(FileSnapshot {
            file,
            length,
            version: source.version(),
        })
    }

    pub fn write_from(
        &self,
        path: &str,
        source: impl Read,
        expected: Option<&ExpectedVersion>,
        options: &WriteOptions,
    ) -> Result<String> {
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        byte_writes::write(&state, path, source, expected, options, Some(self.id))
    }
}
