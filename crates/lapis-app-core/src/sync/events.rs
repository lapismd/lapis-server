mod storage;
mod types;

pub use types::{AcceptedEvent, EventKind, HeadSnapshot, PublishedHead, StoredEvent};

use self::storage::corrupt;
use self::types::{
    EventRecord, MAX_EVENT_DEPENDENCIES, MAX_EVENT_PAYLOAD_BYTES, MAX_HEAD_REVISIONS, validate_ids,
};
use super::{ObjectRepository, invalid, private::open_private_leaf, refused, validate_id};
use crate::vault::VaultRoot;
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use same_file::Handle;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
};

pub struct EventRepository {
    profile: PathBuf,
    profile_identity: Handle,
    vault: String,
    destination: String,
    events: VaultRoot,
    heads: VaultRoot,
    objects: ObjectRepository,
}

impl EventRepository {
    pub fn open(profile: &Path, vault: &str, destination: &str) -> io::Result<Self> {
        let (profile, profile_identity, event_dir, event_path) =
            open_private_leaf(profile, vault, destination, "events")?;
        let (_, _, head_dir, head_path) = open_private_leaf(&profile, vault, destination, "heads")?;
        Ok(Self {
            objects: ObjectRepository::open(&profile, vault, destination)?,
            profile,
            profile_identity,
            vault: vault.to_owned(),
            destination: destination.to_owned(),
            events: VaultRoot::from_capability(event_path, event_dir)?,
            heads: VaultRoot::from_capability(head_path, head_dir)?,
        })
    }

    pub fn objects(&self) -> &ObjectRepository {
        &self.objects
    }

    /// Persists an opaque revision after the Plugins-owned protocol validator
    /// has authenticated its signed identity, authority and dependency list.
    pub fn accept_revision(
        &self,
        id: &str,
        sealed: &[u8],
        object_dependencies: &[String],
        parent_ids: &[String],
    ) -> io::Result<AcceptedEvent> {
        self.verify()?;
        self.validate_input(id, sealed, object_dependencies, parent_ids, false)?;
        self.verify_objects(object_dependencies)?;
        self.verify_revision_graph(parent_ids, Some(id))?;
        self.store(EventRecord {
            id: id.to_owned(),
            vault: self.vault.clone(),
            destination: self.destination.clone(),
            kind: EventKind::Revision,
            device_id: None,
            payload_sha256: format!("{:x}", Sha256::digest(sealed)),
            sealed_payload: STANDARD_NO_PAD.encode(sealed),
            object_dependencies: object_dependencies.to_vec(),
            event_dependencies: parent_ids.to_vec(),
        })
    }

    /// Persists an opaque signed HEAD event, then conditionally makes it the
    /// discoverable tip after Plugins-owned protocol validation.
    pub fn publish_head(
        &self,
        device: &str,
        id: &str,
        sealed: &[u8],
        revision_ids: &[String],
        expected: Option<&str>,
    ) -> io::Result<PublishedHead> {
        self.verify()?;
        validate_id(device)?;
        self.validate_input(id, sealed, &[], revision_ids, true)?;
        if let Some(expected) = expected {
            super::validate_object_id(expected)?;
        }
        self.verify_revision_graph(revision_ids, None)?;
        let stored = self.store(EventRecord {
            id: id.to_owned(),
            vault: self.vault.clone(),
            destination: self.destination.clone(),
            kind: EventKind::Head,
            device_id: Some(device.to_owned()),
            payload_sha256: format!("{:x}", Sha256::digest(sealed)),
            sealed_payload: STANDARD_NO_PAD.encode(sealed),
            object_dependencies: Vec::new(),
            event_dependencies: revision_ids.to_vec(),
        })?;
        let pointer_reused = self.publish_pointer(device, id, expected)?;
        Ok(PublishedHead {
            event_reused: stored.reused,
            pointer_reused,
        })
    }

    pub fn read_event(&self, id: &str) -> io::Result<Option<StoredEvent>> {
        self.verify()?;
        super::validate_object_id(id)?;
        let Some((event, _)) = self.read_record(id)? else {
            return Ok(None);
        };
        self.verify_objects(&event.object_dependencies)?;
        self.verify_revision_graph(
            &event.event_dependencies,
            (event.kind == EventKind::Revision).then_some(event.id.as_str()),
        )?;
        Ok(Some(event))
    }

    pub fn head(&self, device: &str) -> io::Result<Option<HeadSnapshot>> {
        self.verify()?;
        validate_id(device)?;
        let Some((pointer, _)) = self.read_pointer(device)? else {
            return Ok(None);
        };
        let event = self
            .read_event(&pointer.event_id)?
            .ok_or_else(|| corrupt("Discoverable Sync HEAD event is missing"))?;
        if event.kind != EventKind::Head || event.device_id.as_deref() != Some(device) {
            return Err(corrupt("Discoverable Sync HEAD authority does not match"));
        }
        Ok(Some(HeadSnapshot {
            device_id: device.to_owned(),
            revision_ids: event.event_dependencies.clone(),
            event,
        }))
    }

    fn validate_input(
        &self,
        id: &str,
        sealed: &[u8],
        objects: &[String],
        events: &[String],
        head: bool,
    ) -> io::Result<()> {
        super::validate_object_id(id)?;
        if sealed.is_empty() || sealed.len() > MAX_EVENT_PAYLOAD_BYTES {
            return Err(invalid("Invalid sealed Sync event size"));
        }
        validate_ids(objects, false, MAX_EVENT_DEPENDENCIES)?;
        validate_ids(
            events,
            head,
            if head {
                MAX_HEAD_REVISIONS
            } else {
                MAX_EVENT_DEPENDENCIES
            },
        )
    }

    fn verify_revision_graph(&self, roots: &[String], forbidden: Option<&str>) -> io::Result<()> {
        let mut states = HashMap::<String, u8>::new();
        if let Some(forbidden) = forbidden {
            states.insert(forbidden.to_owned(), 1);
        }
        let mut stack: Vec<(String, bool)> = roots.iter().cloned().map(|id| (id, false)).collect();
        while let Some((id, exiting)) = stack.pop() {
            if exiting {
                states.insert(id, 2);
                continue;
            }
            match states.get(&id) {
                Some(1) => return Err(corrupt("Durable Sync event dependency cycle")),
                Some(2) => continue,
                _ => {}
            }
            if states.len() >= MAX_EVENT_DEPENDENCIES {
                return Err(corrupt("Durable Sync event graph exceeds its bound"));
            }
            let event = self
                .read_record(&id)?
                .map(|(event, _)| event)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "Sync event dependency is missing")
                })?;
            if event.kind != EventKind::Revision {
                return Err(corrupt("Sync event dependency is not a revision"));
            }
            self.verify_objects(&event.object_dependencies)?;
            states.insert(id.clone(), 1);
            stack.push((id, true));
            stack.extend(
                event
                    .event_dependencies
                    .into_iter()
                    .map(|parent| (parent, false)),
            );
        }
        Ok(())
    }

    fn verify_objects(&self, ids: &[String]) -> io::Result<()> {
        for id in ids {
            self.objects.verify(id)?;
        }
        Ok(())
    }

    fn verify(&self) -> io::Result<()> {
        let metadata = std::fs::symlink_metadata(&self.profile)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || Handle::from_path(&self.profile)? != self.profile_identity
        {
            return Err(refused("Sync profile changed; admit it again"));
        }
        self.events.verify_physical_root()?;
        self.heads.verify_physical_root()
    }
}
