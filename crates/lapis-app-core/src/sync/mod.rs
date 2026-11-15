//! Durable, profile-private storage used by native Sync services.

use crate::vault::{staged::StagedFile, sync_directory};
use cap_std::{ambient_authority, fs::Dir};
use same_file::Handle;
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
};

pub const TRANSFER_BUFFER_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredObject {
    pub size: u64,
    pub reused: bool,
}

pub struct ObjectRepository {
    profile: PathBuf,
    profile_identity: Handle,
    objects: Dir,
}

impl ObjectRepository {
    pub fn open(profile: &Path, vault_id: &str, destination_id: &str) -> io::Result<Self> {
        validate_id(vault_id)?;
        validate_id(destination_id)?;
        if !profile.is_absolute() {
            return Err(invalid("Sync profile must be absolute"));
        }
        let metadata = std::fs::symlink_metadata(profile)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(refused("Sync profile must be a regular directory"));
        }
        let profile = profile.canonicalize()?;
        let profile_identity = Handle::from_path(&profile)?;
        let mut directory = Dir::open_ambient_dir(&profile, ambient_authority())?;
        for component in [
            "sync",
            "v1",
            "vaults",
            vault_id,
            "destinations",
            destination_id,
            "objects",
        ] {
            directory = private_child(&directory, component)?;
        }
        Ok(Self {
            profile,
            profile_identity,
            objects: directory,
        })
    }

    pub fn put(&self, object_id: &str, source: &mut dyn Read) -> io::Result<StoredObject> {
        self.verify_profile()?;
        validate_object_id(object_id)?;
        if self.objects.symlink_metadata(object_id).is_ok() {
            let size = self.verify_existing(object_id)?;
            return Ok(StoredObject { size, reused: true });
        }
        let mut staged = StagedFile::new(&self.objects)?;
        let (actual, size) = copy_hash(source, &mut staged.file)?;
        if actual != object_id {
            return Err(invalid("Encrypted object hash does not match its identity"));
        }
        staged.file.sync_all()?;
        match staged.publish(OsStr::new(object_id)) {
            Ok(()) => {
                sync_directory(&self.objects)?;
                Ok(StoredObject {
                    size,
                    reused: false,
                })
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let existing = self.verify_existing(object_id)?;
                Ok(StoredObject {
                    size: existing,
                    reused: true,
                })
            }
            Err(error) => Err(error),
        }
    }

    /// Verifies the stored object before streaming it to the caller.
    pub fn copy_verified(&self, object_id: &str, target: &mut dyn Write) -> io::Result<u64> {
        self.verify_profile()?;
        validate_object_id(object_id)?;
        let metadata = self.objects.symlink_metadata(object_id)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(refused("Sync objects must be regular files"));
        }
        let mut file = self.objects.open(object_id)?;
        let (actual, size) = copy_hash(&mut file, &mut io::sink())?;
        if actual != object_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Encrypted object integrity check failed",
            ));
        }
        file.rewind()?;
        let (streamed, streamed_size) = copy_hash(&mut file, target)?;
        if streamed != object_id || streamed_size != size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Encrypted object changed while it was being streamed",
            ));
        }
        Ok(size)
    }

    fn verify_existing(&self, object_id: &str) -> io::Result<u64> {
        let metadata = self.objects.symlink_metadata(object_id)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(refused("Sync objects must be regular files"));
        }
        let mut file = self.objects.open(object_id)?;
        let mut sink = io::sink();
        let (actual, size) = copy_hash(&mut file, &mut sink)?;
        if actual != object_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Existing encrypted object is corrupt",
            ));
        }
        Ok(size)
    }

    fn verify_profile(&self) -> io::Result<()> {
        let metadata = std::fs::symlink_metadata(&self.profile)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || Handle::from_path(&self.profile)? != self.profile_identity
        {
            return Err(refused("Sync profile changed; admit it again"));
        }
        Ok(())
    }
}

fn private_child(parent: &Dir, name: &str) -> io::Result<Dir> {
    let mut created = false;
    match parent.symlink_metadata(name) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(refused("Sync repository components must be directories"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => match parent.create_dir(name) {
            Ok(()) => created = true,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        },
        Err(error) => return Err(error),
    }
    let metadata = parent.symlink_metadata(name)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(refused("Sync repository components must be directories"));
    }
    #[cfg(unix)]
    {
        use cap_std::fs::PermissionsExt;
        parent.set_permissions(name, cap_std::fs::Permissions::from_mode(0o700))?;
    }
    if created {
        sync_directory(parent)?;
    }
    let child = parent.open_dir(name)?;
    verify_opened_child(&child, &parent.symlink_metadata(name)?)?;
    Ok(child)
}

#[cfg(unix)]
fn verify_opened_child(child: &Dir, expected: &cap_std::fs::Metadata) -> io::Result<()> {
    use cap_std::fs::MetadataExt;
    let actual = child.dir_metadata()?;
    if actual.dev() != expected.dev() || actual.ino() != expected.ino() {
        return Err(refused(
            "Sync repository component changed during admission",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_opened_child(child: &Dir, _: &cap_std::fs::Metadata) -> io::Result<()> {
    if !child.dir_metadata()?.is_dir() {
        return Err(refused(
            "Sync repository component changed during admission",
        ));
    }
    Ok(())
}

fn validate_id(value: &str) -> io::Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(invalid("Invalid Sync repository identity"));
    }
    Ok(())
}

fn validate_object_id(value: &str) -> io::Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid("Invalid encrypted object identity"));
    }
    Ok(())
}

fn copy_hash(source: &mut dyn Read, target: &mut dyn Write) -> io::Result<(String, u64)> {
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = vec![0_u8; TRANSFER_BUFFER_BYTES];
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
        target.write_all(&buffer[..read])?;
        size = size
            .checked_add(read as u64)
            .ok_or_else(|| invalid("Encrypted object is too large"))?;
    }
    Ok((format!("{:x}", hash.finalize()), size))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn refused(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}
