use super::{ArrivalKind, ArrivalRepository, apply_record};
use crate::{
    sync::{invalid, refused},
    vault::{VaultRoot, WriteOptions},
};
use same_file::Handle;
use std::io::{self, Read};
use uuid::Uuid;

use super::types::{ArrivalRecord, ArrivalState, MAX_RECORD_BYTES};

impl ArrivalRepository {
    pub(super) fn initialize(&self) -> io::Result<()> {
        if self.arrivals.open_file("state.json").is_ok() {
            self.read_state()?;
            return Ok(());
        }
        let state = ArrivalState {
            vault: self.vault.clone(),
            destination: self.destination.clone(),
            cursor_generation: Uuid::new_v4(),
            last_sequence: 0,
            membership_generation: None,
            membership_event_id: None,
        };
        let bytes = serde_json::to_vec(&state)
            .map_err(|_| invalid("Sync arrival state is not serializable"))?;
        match self
            .arrivals
            .create_file("state.json", &bytes[..], &WriteOptions::default())
        {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                self.read_state().map(|_| ())
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn recover(&self) -> io::Result<()> {
        for _ in 0..1024 {
            let (mut state, previous) = self.read_state()?;
            let next = state.last_sequence + 1;
            let Some(record) = self.read_sequence(next)? else {
                return Ok(());
            };
            match self.read_identity(&record.event_id)? {
                Some(existing) if existing != record => {
                    return Err(corrupt("Conflicting Sync arrival identity"));
                }
                Some(_) => {}
                None => {
                    let bytes = self.serialize(&record)?;
                    self.identities.create_file(
                        &identity_path(&record.event_id),
                        &bytes[..],
                        &WriteOptions::default(),
                    )?;
                }
            }
            if record.kind == ArrivalKind::Membership {
                let expected = state.membership_generation.map_or(0, |value| value + 1);
                if record.membership_generation != Some(expected)
                    || record.predecessor != state.membership_event_id
                {
                    return Err(corrupt("Durable Sync membership chain is invalid"));
                }
            }
            apply_record(&mut state, &record);
            self.write_state(&state, &previous)?;
        }
        Err(corrupt("Sync arrival recovery exceeds its bound"))
    }

    pub(super) fn read_state(&self) -> io::Result<(ArrivalState, Vec<u8>)> {
        let bytes = read_bounded(&self.arrivals, "state.json", 4096)?
            .ok_or_else(|| corrupt("Durable Sync arrival state is missing"))?;
        let state: ArrivalState = serde_json::from_slice(&bytes)
            .map_err(|_| corrupt("Durable Sync arrival state is unreadable"))?;
        state
            .validate(&self.vault, &self.destination)
            .map_err(|_| corrupt("Durable Sync arrival state failed authority validation"))?;
        Ok((state, bytes))
    }

    pub(super) fn write_state(&self, state: &ArrivalState, previous: &[u8]) -> io::Result<()> {
        state.validate(&self.vault, &self.destination)?;
        let bytes = serde_json::to_vec(state)
            .map_err(|_| invalid("Sync arrival state is not serializable"))?;
        self.arrivals.write_if_unchanged(
            "state.json",
            &bytes[..],
            Some(previous),
            &WriteOptions::default(),
        )
    }

    pub(super) fn read_sequence(&self, sequence: u64) -> io::Result<Option<ArrivalRecord>> {
        self.read_record(&self.arrivals, &sequence_path(sequence), Some(sequence))
    }

    pub(super) fn read_identity(&self, event_id: &str) -> io::Result<Option<ArrivalRecord>> {
        self.read_record(&self.identities, &identity_path(event_id), None)
    }

    fn read_record(
        &self,
        root: &VaultRoot,
        path: &str,
        sequence: Option<u64>,
    ) -> io::Result<Option<ArrivalRecord>> {
        let Some(bytes) = read_bounded(root, path, MAX_RECORD_BYTES)? else {
            return Ok(None);
        };
        let record: ArrivalRecord = serde_json::from_slice(&bytes)
            .map_err(|_| corrupt("Durable Sync arrival is unreadable"))?;
        record
            .validate(&self.vault, &self.destination)
            .map_err(|_| corrupt("Durable Sync arrival failed validation"))?;
        if sequence.is_some_and(|value| record.sequence != value) {
            return Err(corrupt("Durable Sync arrival sequence changed"));
        }
        Ok(Some(record))
    }

    pub(super) fn serialize(&self, record: &ArrivalRecord) -> io::Result<Vec<u8>> {
        let bytes =
            serde_json::to_vec(record).map_err(|_| invalid("Sync arrival is not serializable"))?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(invalid("Sync arrival exceeds its bound"));
        }
        Ok(bytes)
    }

    pub(super) fn verify(&self) -> io::Result<()> {
        let metadata = std::fs::symlink_metadata(&self.profile)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || Handle::from_path(&self.profile)? != self.profile_identity
        {
            return Err(refused("Sync profile changed; admit it again"));
        }
        self.arrivals.verify_physical_root()?;
        self.identities.verify_physical_root()?;
        if Handle::from_file(self.arrivals.open_file(".owner.lock")?.into_std())?
            != self.lock_identity
        {
            return Err(refused("Sync arrival owner changed"));
        }
        let _ = &self.lock;
        Ok(())
    }
}

pub(super) fn sequence_path(sequence: u64) -> String {
    format!("{sequence:020}.json")
}
pub(super) fn identity_path(event_id: &str) -> String {
    format!("{event_id}.json")
}
fn read_bounded(root: &VaultRoot, path: &str, max: usize) -> io::Result<Option<Vec<u8>>> {
    let file = match root.open_file(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(max as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err(corrupt("Durable Sync arrival exceeds its bound"));
    }
    Ok(Some(bytes))
}
fn corrupt(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
