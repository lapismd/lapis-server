use super::{MAX_FILE_BYTES, invalid};
use cap_std::fs::{Dir, OpenOptions};
use std::{
    ffi::OsStr,
    fmt,
    io::{self, Read},
};

#[derive(Debug)]
pub struct TextWriteConflict {
    path: String,
}
impl fmt::Display for TextWriteConflict {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "File changed on disk: {}", self.path)
    }
}
impl std::error::Error for TextWriteConflict {}

fn conflict(path: &str) -> io::Error {
    io::Error::other(TextWriteConflict {
        path: path.to_owned(),
    })
}

#[derive(Clone, Copy)]
pub(super) enum WriteCondition<'a> {
    Any,
    Contents(Option<&'a [u8]>),
}

impl WriteCondition<'_> {
    pub(super) fn verify(self, parent: &Dir, name: &OsStr, path: &str) -> io::Result<()> {
        let Self::Contents(expected) = self else {
            return Ok(());
        };
        if expected.is_some_and(|bytes| bytes.len() as u64 > MAX_FILE_BYTES) {
            return Err(invalid("Expected contents exceed the 64 MiB limit"));
        }
        let exists = match parent.symlink_metadata(name) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Symlinks are unavailable",
                ));
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(invalid("Destination is not a regular file"));
            }
            Ok(_) => true,
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(error),
        };
        match (exists, expected) {
            (false, None) => return Ok(()),
            (false, Some(_)) | (true, None) => return Err(conflict(path)),
            _ => {}
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = parent.open_with(name, &options).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                conflict(path)
            } else {
                error
            }
        })?;
        if !file.metadata()?.is_file() {
            return Err(invalid("Only regular files can be read"));
        }
        let mut actual = Vec::new();
        file.take(MAX_FILE_BYTES + 1).read_to_end(&mut actual)?;
        if actual.len() as u64 > MAX_FILE_BYTES {
            return Err(conflict(path));
        }
        if expected != Some(actual.as_slice()) {
            return Err(conflict(path));
        }
        Ok(())
    }
}
