mod storage;
mod types;

pub use types::{
    ArrivalCursor, ArrivalItem, ArrivalKind, ArrivalPage, CatchUpOutcome, RecordedArrival,
};

use self::storage::{identity_path, sequence_path};
use self::types::{ArrivalRecord, ArrivalState, MAX_MEMBERSHIP_BYTES, MAX_PAGE_SIZE};
use super::{invalid, open_private_leaf, validate_object_id};
use crate::vault::{VaultRoot, WriteOptions};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use same_file::Handle;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub struct ArrivalRepository {
    profile: PathBuf,
    profile_identity: Handle,
    arrivals: VaultRoot,
    identities: VaultRoot,
    lock: File,
    lock_identity: Handle,
    vault: String,
    destination: String,
}

impl ArrivalRepository {
    pub fn open(profile: &Path, vault: &str, destination: &str) -> io::Result<Self> {
        let (profile, profile_identity, directory, path) =
            open_private_leaf(profile, vault, destination, "arrivals")?;
        let (_, _, id_directory, id_path) =
            open_private_leaf(&profile, vault, destination, "arrival-ids")?;
        let arrivals = VaultRoot::from_capability(path, directory)?;
        match arrivals.create_file(".owner.lock", &b""[..], &WriteOptions::default()) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let lock = arrivals.open_file(".owner.lock")?.into_std();
        lock.try_lock()?;
        let repository = Self {
            profile,
            profile_identity,
            identities: VaultRoot::from_capability(id_path, id_directory)?,
            lock_identity: Handle::from_file(lock.try_clone()?)?,
            lock,
            arrivals,
            vault: vault.to_owned(),
            destination: destination.to_owned(),
        };
        repository.initialize()?;
        repository.recover()?;
        Ok(repository)
    }

    /// Records a revision or HEAD only after `EventRepository` has durably
    /// accepted the same caller-validated event identity and dependencies.
    pub fn record_event(&self, kind: ArrivalKind, event_id: &str) -> io::Result<RecordedArrival> {
        if kind == ArrivalKind::Membership {
            return Err(invalid("Membership arrivals require their durable payload"));
        }
        self.append(ArrivalRecord {
            sequence: 0,
            vault: self.vault.clone(),
            destination: self.destination.clone(),
            kind,
            event_id: event_id.to_owned(),
            membership_generation: None,
            predecessor: None,
            payload_sha256: None,
            sealed_payload: None,
        })
    }

    /// Retains an opaque owner-authorized membership event after protocol validation.
    pub fn record_membership(
        &self,
        event_id: &str,
        generation: u64,
        predecessor: Option<&str>,
        sealed: &[u8],
    ) -> io::Result<RecordedArrival> {
        if sealed.is_empty() || sealed.len() > MAX_MEMBERSHIP_BYTES {
            return Err(invalid("Invalid sealed Sync membership size"));
        }
        self.append(ArrivalRecord {
            sequence: 0,
            vault: self.vault.clone(),
            destination: self.destination.clone(),
            kind: ArrivalKind::Membership,
            event_id: event_id.to_owned(),
            membership_generation: Some(generation),
            predecessor: predecessor.map(str::to_owned),
            payload_sha256: Some(format!("{:x}", Sha256::digest(sealed))),
            sealed_payload: Some(STANDARD_NO_PAD.encode(sealed)),
        })
    }

    pub fn read_membership(&self, event_id: &str) -> io::Result<Option<Vec<u8>>> {
        self.verify()?;
        validate_object_id(event_id)?;
        let Some(record) = self.read_identity(event_id)? else {
            return Ok(None);
        };
        if record.kind != ArrivalKind::Membership {
            return Err(invalid("Sync arrival is not membership history"));
        }
        record.open_membership().map(Some)
    }

    pub fn page(&self, cursor: Option<&ArrivalCursor>, limit: usize) -> io::Result<CatchUpOutcome> {
        self.verify()?;
        if !(1..=MAX_PAGE_SIZE).contains(&limit) {
            return Err(invalid("Invalid Sync catch-up page size"));
        }
        let (state, _) = self.read_state()?;
        let cursor = cursor.cloned().unwrap_or(ArrivalCursor {
            generation: state.cursor_generation,
            snapshot: state.last_sequence,
            after: 0,
        });
        if cursor.generation != state.cursor_generation {
            return Ok(CatchUpOutcome::RestartRequired);
        }
        if cursor.after > cursor.snapshot || cursor.snapshot > state.last_sequence {
            return Err(invalid("Invalid Sync catch-up cursor"));
        }
        let end = cursor
            .after
            .saturating_add(limit as u64)
            .min(cursor.snapshot);
        let mut items = Vec::with_capacity((end - cursor.after) as usize);
        for sequence in cursor.after + 1..=end {
            let record = self.read_sequence(sequence)?.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Durable Sync arrival is missing",
                )
            })?;
            items.push(record.item());
        }
        let complete = end == cursor.snapshot;
        let current = ArrivalCursor {
            after: end,
            ..cursor
        };
        Ok(CatchUpOutcome::Page(ArrivalPage {
            items,
            cursor: current.clone(),
            next_cursor: (!complete).then_some(current),
            complete,
        }))
    }

    pub fn expire_cursors_for_rebuild(&self) -> io::Result<()> {
        self.verify()?;
        let (mut state, previous) = self.read_state()?;
        state.cursor_generation = Uuid::new_v4();
        self.write_state(&state, &previous)
    }

    fn append(&self, mut record: ArrivalRecord) -> io::Result<RecordedArrival> {
        self.verify()?;
        validate_object_id(&record.event_id)?;
        self.recover()?;
        if let Some(existing) = self.read_identity(&record.event_id)? {
            record.sequence = existing.sequence;
            if existing != record {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Sync arrival retry does not match durable bytes",
                ));
            }
            return Ok(RecordedArrival {
                item: existing.item(),
                reused: true,
            });
        }
        let (mut state, previous) = self.read_state()?;
        if record.kind == ArrivalKind::Membership {
            let expected_generation = state.membership_generation.map_or(0, |value| value + 1);
            if record.membership_generation != Some(expected_generation)
                || record.predecessor != state.membership_event_id
            {
                return Err(invalid("Sync membership predecessor or generation changed"));
            }
        }
        record.sequence = state
            .last_sequence
            .checked_add(1)
            .ok_or_else(|| invalid("Sync arrival sequence is exhausted"))?;
        record.validate(&self.vault, &self.destination)?;
        let bytes = self.serialize(&record)?;
        self.arrivals.create_file(
            &sequence_path(record.sequence),
            &bytes[..],
            &WriteOptions::default(),
        )?;
        self.identities.create_file(
            &identity_path(&record.event_id),
            &bytes[..],
            &WriteOptions::default(),
        )?;
        apply_record(&mut state, &record);
        self.write_state(&state, &previous)?;
        self.verify()?;
        Ok(RecordedArrival {
            item: record.item(),
            reused: false,
        })
    }
}

fn apply_record(state: &mut ArrivalState, record: &ArrivalRecord) {
    state.last_sequence = record.sequence;
    if record.kind == ArrivalKind::Membership {
        state.membership_generation = record.membership_generation;
        state.membership_event_id = Some(record.event_id.clone());
    }
}
