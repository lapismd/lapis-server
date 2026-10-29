use crate::vault::{VaultRoot, WriteOptions, sync_directory};
use cap_std::fs::MetadataExt;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::File,
    io::{self, Read},
    marker::PhantomData,
    path::Path,
};
use uuid::Uuid;

/// Private record IO shared by independently typed native journals.
pub(crate) trait StoredRecord: Serialize + DeserializeOwned {
    const KIND: &'static str;
    const MAX_RECORD_BYTES: usize;
    const MAX_RECORDS: usize;
    fn id(&self) -> Uuid;
    fn validate(&self, vault: Uuid, id: Uuid) -> io::Result<()>;
}

pub(crate) struct RecordStore<T: StoredRecord> {
    record: PhantomData<T>,
    profile: VaultRoot,
    root: VaultRoot,
    lock: File,
    retired: bool,
    pub vault: Uuid,
    pub generation: Uuid,
}
impl<T: StoredRecord> RecordStore<T> {
    fn invalid(message: &str) -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Native {} {message}", T::KIND),
        )
    }
    pub fn open(directory: &Path, vault: Uuid, content: &VaultRoot) -> io::Result<Self> {
        if vault.is_nil()
            || !directory.is_absolute()
            || !std::fs::symlink_metadata(directory)?.is_dir()
        {
            return Err(Self::invalid("storage requires a physical profile"));
        }
        content.verify_physical_root()?;
        let profile = VaultRoot::open(directory)?;
        if profile.path().starts_with(content.path()) || content.path().starts_with(profile.path())
        {
            return Err(Self::invalid("authority must remain outside vault content"));
        }
        let name = vault.to_string();
        match profile.mkdir(&name, false) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let root = profile.open_child(&name)?;
        sync_directory(&profile.directory_handle()?)?;
        match root.create_file(".owner.lock", &b""[..], &WriteOptions::default()) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let lock = root.open_file(".owner.lock")?.into_std();
        lock.try_lock().map_err(io::Error::from)?;
        let value = Self {
            record: PhantomData,
            profile,
            root,
            lock,
            retired: false,
            vault,
            generation: Uuid::new_v4(),
        };
        value.verify()?;
        Ok(value)
    }
    pub fn verify(&self) -> io::Result<()> {
        use std::os::unix::fs::MetadataExt as _;
        if self.retired {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                format!("Native {} writer is retired", T::KIND),
            ));
        }
        self.profile.verify_physical_root()?;
        self.root.verify_physical_root()?;
        let current = self.root.entry_metadata(".owner.lock")?;
        let held = self.lock.metadata()?;
        if !current.is_file() || current.dev() != held.dev() || current.ino() != held.ino() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("Native {} writer changed", T::KIND),
            ));
        }
        Ok(())
    }
    pub fn retire(&mut self) -> io::Result<()> {
        if !self.retired {
            self.verify()?;
            self.lock.unlock()?;
            self.retired = true;
        }
        Ok(())
    }
    pub fn ids(&self) -> io::Result<Vec<Uuid>> {
        self.verify()?;
        let mut ids = Vec::new();
        let directory = self.root.directory_handle()?;
        for (index, entry) in directory.entries()?.enumerate() {
            // Records, the writer lock and one interrupted temporary publication.
            if index >= T::MAX_RECORDS + 2 {
                return Err(Self::invalid("journal exceeds its supported catalogue"));
            }
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| Self::invalid("filename is not UTF-8"))?;
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            if !entry.file_type()?.is_file() {
                return Err(Self::invalid("record is not a regular file"));
            }
            let parsed = Uuid::parse_str(id).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid native {} filename", T::KIND),
                )
            })?;
            if parsed.get_version_num() != 4
                || parsed.to_string() != id
                || ids.len() >= T::MAX_RECORDS
            {
                return Err(Self::invalid("journal exceeds its supported catalogue"));
            }
            ids.push(parsed);
        }
        ids.sort();
        Ok(ids)
    }
    pub fn read(&self, id: Uuid) -> io::Result<Option<(T, Vec<u8>)>> {
        self.verify()?;
        let file = match self.root.open_file(&format!("{id}.json")) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut bytes = Vec::new();
        file.take(T::MAX_RECORD_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > T::MAX_RECORD_BYTES {
            return Err(Self::invalid("record exceeds its bound"));
        }
        let record: T =
            serde_json::from_slice(&bytes).map_err(|_| Self::invalid("record is unreadable"))?;
        record.validate(self.vault, id)?;
        Ok(Some((record, bytes)))
    }
    pub fn write(&self, record: &T, expected: Option<&[u8]>) -> io::Result<Vec<u8>> {
        self.verify()?;
        record.validate(self.vault, record.id())?;
        let bytes =
            serde_json::to_vec(record).map_err(|_| Self::invalid("record is not serializable"))?;
        if bytes.len() > T::MAX_RECORD_BYTES {
            return Err(Self::invalid("record exceeds its bound"));
        }
        let path = format!("{}.json", record.id());
        match expected {
            Some(expected) => self.root.write_if_unchanged(
                &path,
                &bytes[..],
                Some(expected),
                &WriteOptions::default(),
            )?,
            None => self
                .root
                .create_file(&path, &bytes[..], &WriteOptions::default())?,
        }
        self.verify()?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests;
