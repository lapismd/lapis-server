use super::{MAX_FILE_BYTES, MAX_RANGE_BYTES, invalid};
use cap_std::{ambient_authority, fs::Dir};
use serde::Serialize;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub struct VaultRoot {
    pub(crate) dir: Dir,
    path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct FileStat {
    pub r#type: &'static str,
    pub size: u64,
    pub ctime: f64,
    pub mtime: f64,
}

#[derive(Debug, Serialize)]
pub struct ListedFiles {
    pub files: Vec<String>,
    pub folders: Vec<String>,
}

impl VaultRoot {
    pub fn open(path: &Path) -> io::Result<Self> {
        if !path.is_absolute() {
            return Err(invalid("Folder selection must be absolute"));
        }
        let path = path.canonicalize()?;
        let dir = Dir::open_ambient_dir(&path, ambient_authority())?;
        Ok(Self { dir, path })
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            dir: self.dir.try_clone()?,
            path: self.path.clone(),
        })
    }

    pub fn physical_identity(&self) -> io::Result<(u64, u64)> {
        use cap_std::fs::MetadataExt;
        let metadata = self.dir.dir_metadata()?;
        Ok((metadata.dev(), metadata.ino()))
    }

    pub fn verify_physical_root(&self) -> io::Result<()> {
        use cap_std::fs::MetadataExt;
        use std::os::unix::fs::MetadataExt as _;
        let expected = self.dir.dir_metadata()?;
        let actual = std::fs::symlink_metadata(&self.path)?;
        if !actual.is_dir() || actual.dev() != expected.dev() || actual.ino() != expected.ino() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Vault folder changed; select it again",
            ));
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn create_child(&self, name: &str) -> io::Result<Self> {
        self.mkdir(name, false)?;
        let dir = self.dir.open_dir(self.checked(name, false)?)?;
        Ok(Self {
            dir,
            path: self.path.join(name),
        })
    }

    pub(crate) fn open_child(&self, name: &str) -> io::Result<Self> {
        self.verify_physical_root()?;
        let relative = self.checked(name, false)?;
        let child = Self {
            dir: self.dir.open_dir(&relative)?,
            path: self.path.join(relative),
        };
        child.verify_physical_root()?;
        Ok(child)
    }

    pub(crate) fn checked(&self, path: &str, root_allowed: bool) -> io::Result<PathBuf> {
        if path.is_empty() && root_allowed {
            return Ok(PathBuf::from("."));
        }
        if path.is_empty() || path.len() > 4096 || path.contains(['\\', '\0', ':']) {
            return Err(invalid("Invalid relative vault path"));
        }
        let mut relative = PathBuf::new();
        for part in path.split('/') {
            if part.is_empty() || part == "." || part == ".." {
                return Err(invalid("Invalid relative vault path"));
            }
            relative.push(part);
            match self.dir.symlink_metadata(&relative) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "Symlinks are unavailable",
                    ));
                }
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                _ => {}
            }
        }
        Ok(relative)
    }

    pub fn stat(&self, path: &str) -> io::Result<Option<FileStat>> {
        let relative = self.checked(path, true)?;
        let metadata = match self.dir.metadata(relative) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let kind = if metadata.is_dir() {
            "folder"
        } else if metadata.is_file() {
            "file"
        } else {
            return Err(invalid("Only regular files and directories are available"));
        };
        let millis = |time: io::Result<cap_std::time::SystemTime>| {
            time.ok()
                .and_then(|time| time.into_std().duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0.0, |duration| duration.as_millis() as f64)
        };
        Ok(Some(FileStat {
            r#type: kind,
            size: metadata.len(),
            ctime: millis(metadata.created().or_else(|_| metadata.modified())),
            mtime: millis(metadata.modified()),
        }))
    }

    pub(crate) fn open_file(&self, path: &str) -> io::Result<cap_std::fs::File> {
        let mut options = cap_std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = self.dir.open_with(self.checked(path, false)?, &options)?;
        if !file.metadata()?.is_file() {
            return Err(invalid("Only regular files can be read"));
        }
        Ok(file)
    }

    pub fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        let mut file = self.open_file(path)?.take(MAX_FILE_BYTES + 1);
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(invalid("File exceeds the 64 MiB limit"));
        }
        Ok(bytes)
    }

    pub fn read_text(&self, path: &str) -> io::Result<String> {
        String::from_utf8(self.read(path)?).map_err(|_| invalid("File is not valid UTF-8"))
    }

    pub fn read_range(&self, path: &str, offset: u64, length: usize) -> io::Result<Vec<u8>> {
        if length > MAX_RANGE_BYTES {
            return Err(invalid("Read range exceeds 256 KiB"));
        }
        let mut file = self.open_file(path)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = Vec::new();
        file.take(length as u64).read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    pub fn list(&self, path: &str) -> io::Result<ListedFiles> {
        let mut result = ListedFiles {
            files: Vec::new(),
            folders: Vec::new(),
        };
        let directory = self.dir.open_dir(self.checked(path, true)?)?;
        for (index, entry) in directory.entries()?.enumerate() {
            if index >= 2000 {
                return Err(invalid("Directory exceeds the 2,000 entry limit"));
            }
            let entry = entry?;
            let kind = entry.file_type()?;
            if !kind.is_dir() && !kind.is_file() {
                continue;
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid("Filename is not UTF-8"))?;
            if kind.is_dir() {
                result.folders.push(name);
            } else {
                result.files.push(name);
            }
        }
        result.files.sort();
        result.folders.sort();
        Ok(result)
    }

    pub fn mkdir(&self, path: &str, recursive: bool) -> io::Result<()> {
        let relative = self.checked(path, false)?;
        if recursive {
            self.dir.create_dir_all(relative)
        } else {
            self.dir.create_dir(relative)
        }
    }
}
