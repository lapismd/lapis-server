mod types;

pub use types::{OperationKind, OperationPhase, OperationRecord, StageOutcome};

use self::types::{
    MAX_RECORD_BYTES, MAX_SEALED_BYTES, allowed_transition, decode_sealed, validate_request_id,
};
use super::{invalid, private::open_private_leaf, refused};
use crate::vault::{VaultRoot, WriteOptions};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use same_file::Handle;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub struct OperationJournal {
    profile: PathBuf,
    profile_identity: Handle,
    root: VaultRoot,
    lock: File,
    lock_identity: Handle,
    retired: bool,
    vault: Uuid,
    destination: String,
}

impl OperationJournal {
    pub fn open(
        profile: &Path,
        content: &VaultRoot,
        vault: Uuid,
        destination: &str,
    ) -> io::Result<Self> {
        if vault.is_nil() {
            return Err(invalid("Invalid Sync vault identity"));
        }
        content.verify_physical_root()?;
        let admitted_profile = profile.canonicalize()?;
        if admitted_profile.starts_with(content.path())
            || content.path().starts_with(&admitted_profile)
        {
            return Err(refused("Sync journal must remain outside vault content"));
        }
        let (profile, profile_identity, directory, path) =
            open_private_leaf(profile, &vault.to_string(), destination, "journal")?;
        let root = VaultRoot::from_capability(path, directory)?;
        match root.create_file(".owner.lock", &b""[..], &WriteOptions::default()) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let lock = root.open_file(".owner.lock")?.into_std();
        lock.try_lock()?;
        let lock_identity = Handle::from_file(lock.try_clone()?)?;
        let journal = Self {
            profile,
            profile_identity,
            root,
            lock,
            lock_identity,
            retired: false,
            vault,
            destination: destination.to_owned(),
        };
        journal.verify()?;
        Ok(journal)
    }

    pub fn stage(
        &self,
        id: Uuid,
        request_id: &str,
        kind: OperationKind,
        sealed: &[u8],
    ) -> io::Result<StageOutcome> {
        self.verify()?;
        validate_request_id(request_id)?;
        if sealed.is_empty() || sealed.len() > MAX_SEALED_BYTES {
            return Err(invalid("Invalid sealed Sync payload size"));
        }
        let encoded = STANDARD_NO_PAD.encode(sealed);
        let hash = format!("{:x}", Sha256::digest(sealed));
        if let Some((record, _)) = self.read_record(id)? {
            if record.request_id != request_id
                || record.kind != kind
                || record.ciphertext_sha256 != hash
                || record.sealed_payload != encoded
            {
                return Err(corrupt("Sync retry identity does not match durable work"));
            }
            return Ok(StageOutcome {
                record,
                reused: true,
            });
        }
        let record = OperationRecord {
            id,
            vault: self.vault,
            destination: self.destination.clone(),
            request_id: request_id.to_owned(),
            kind,
            phase: OperationPhase::Staged,
            ciphertext_sha256: hash,
            sealed_payload: encoded,
        };
        let bytes = self.serialize(&record)?;
        self.root
            .create_file(&record_path(id), &bytes[..], &WriteOptions::default())?;
        self.verify()?;
        Ok(StageOutcome {
            record,
            reused: false,
        })
    }

    pub fn read(&self, id: Uuid) -> io::Result<Option<OperationRecord>> {
        Ok(self.read_record(id)?.map(|(record, _)| record))
    }

    pub fn open_sealed(&self, record: &OperationRecord) -> io::Result<Vec<u8>> {
        self.verify()?;
        record.validate(self.vault, &self.destination, record.id)?;
        decode_sealed(record)
    }

    pub fn transition(
        &self,
        id: Uuid,
        expected: OperationPhase,
        next: OperationPhase,
    ) -> io::Result<OperationRecord> {
        self.verify()?;
        let Some((mut record, previous)) = self.read_record(id)? else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Durable Sync operation was not found",
            ));
        };
        if record.phase != expected || !allowed_transition(expected, next) {
            return Err(invalid("Invalid durable Sync phase transition"));
        }
        record.phase = next;
        let bytes = self.serialize(&record)?;
        self.root.write_if_unchanged(
            &record_path(id),
            &bytes[..],
            Some(&previous),
            &WriteOptions::default(),
        )?;
        self.verify()?;
        Ok(record)
    }

    pub fn retire(&mut self) -> io::Result<()> {
        if !self.retired {
            self.verify()?;
            self.lock.unlock()?;
            self.retired = true;
        }
        Ok(())
    }

    fn read_record(&self, id: Uuid) -> io::Result<Option<(OperationRecord, Vec<u8>)>> {
        self.verify()?;
        if id.get_version_num() != 4 {
            return Err(invalid("Invalid durable Sync operation identity"));
        }
        let file = match self.root.open_file(&record_path(id)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        file.take(MAX_RECORD_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(corrupt("Durable Sync operation exceeds its bound"));
        }
        let record: OperationRecord = serde_json::from_slice(&bytes)
            .map_err(|_| corrupt("Durable Sync operation is unreadable"))?;
        record
            .validate(self.vault, &self.destination, id)
            .map_err(|_| corrupt("Durable Sync operation failed authority validation"))?;
        decode_sealed(&record)
            .map_err(|_| corrupt("Durable Sync payload failed integrity validation"))?;
        Ok(Some((record, bytes)))
    }

    fn serialize(&self, record: &OperationRecord) -> io::Result<Vec<u8>> {
        record.validate(self.vault, &self.destination, record.id)?;
        let bytes = serde_json::to_vec(record)
            .map_err(|_| invalid("Durable Sync operation is not serializable"))?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(invalid("Durable Sync operation exceeds its bound"));
        }
        Ok(bytes)
    }

    fn verify(&self) -> io::Result<()> {
        if self.retired {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "Sync operation journal is retired",
            ));
        }
        let metadata = std::fs::symlink_metadata(&self.profile)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || Handle::from_path(&self.profile)? != self.profile_identity
        {
            return Err(refused("Sync profile changed; admit it again"));
        }
        self.root.verify_physical_root()?;
        let current = self.root.open_file(".owner.lock")?.into_std();
        if Handle::from_file(current)? != self.lock_identity {
            return Err(refused("Sync operation journal owner changed"));
        }
        Ok(())
    }
}

fn record_path(id: Uuid) -> String {
    format!("{id}.json")
}

fn corrupt(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
