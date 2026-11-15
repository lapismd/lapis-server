use super::types::{AcceptedEvent, EventRecord, HeadPointer, MAX_EVENT_RECORD_BYTES};
use super::{EventRepository, StoredEvent};
use crate::sync::invalid;
use crate::vault::{VaultRoot, WriteOptions};
use std::io::{self, Read};

impl EventRepository {
    pub(super) fn store(&self, record: EventRecord) -> io::Result<AcceptedEvent> {
        if let Some((existing, _)) = self.read_record(&record.id)? {
            return compare_existing(record, existing);
        }
        let event = record.open()?;
        let bytes = self.serialize(&record)?;
        match self.events.create_file(
            &event_path(&record.id),
            &bytes[..],
            &WriteOptions::default(),
        ) {
            Ok(()) => {
                self.verify()?;
                Ok(AcceptedEvent {
                    event,
                    reused: false,
                })
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let existing = self
                    .read_record(&record.id)?
                    .map(|(event, _)| event)
                    .ok_or(error)?;
                compare_existing(record, existing)
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn publish_pointer(
        &self,
        device: &str,
        event_id: &str,
        expected: Option<&str>,
    ) -> io::Result<bool> {
        let current = self.read_pointer(device)?;
        if current.as_ref().map(pointer_id) == Some(event_id) {
            return Ok(true);
        }
        if current.as_ref().map(pointer_id) != expected {
            return Err(head_changed());
        }
        let pointer = HeadPointer {
            vault: self.vault.clone(),
            destination: self.destination.clone(),
            device_id: device.to_owned(),
            event_id: event_id.to_owned(),
        };
        let bytes = serde_json::to_vec(&pointer)
            .map_err(|_| invalid("Sync HEAD pointer is not serializable"))?;
        let path = pointer_path(device);
        let result = match current {
            Some((_, previous)) => self.heads.write_if_unchanged(
                &path,
                &bytes[..],
                Some(&previous),
                &WriteOptions::default(),
            ),
            None => self
                .heads
                .create_file(&path, &bytes[..], &WriteOptions::default()),
        };
        match result {
            Ok(()) => {
                self.verify()?;
                Ok(false)
            }
            Err(error) => {
                if self.read_pointer(device)?.as_ref().map(pointer_id) == Some(event_id) {
                    return Ok(true);
                }
                if matches!(
                    error.kind(),
                    io::ErrorKind::AlreadyExists | io::ErrorKind::Other
                ) {
                    Err(head_changed())
                } else {
                    Err(error)
                }
            }
        }
    }

    pub(super) fn read_record(&self, id: &str) -> io::Result<Option<(StoredEvent, Vec<u8>)>> {
        let Some(bytes) = read_bounded(&self.events, &event_path(id), MAX_EVENT_RECORD_BYTES)?
        else {
            return Ok(None);
        };
        let record: EventRecord = serde_json::from_slice(&bytes)
            .map_err(|_| corrupt("Durable Sync event is unreadable"))?;
        record
            .validate(&self.vault, &self.destination, id)
            .map_err(|_| corrupt("Durable Sync event failed authority validation"))?;
        let event = record
            .open()
            .map_err(|_| corrupt("Durable Sync event failed integrity validation"))?;
        Ok(Some((event, bytes)))
    }

    pub(super) fn read_pointer(&self, device: &str) -> io::Result<Option<(HeadPointer, Vec<u8>)>> {
        let Some(bytes) = read_bounded(&self.heads, &pointer_path(device), 1024)? else {
            return Ok(None);
        };
        let pointer: HeadPointer = serde_json::from_slice(&bytes)
            .map_err(|_| corrupt("Durable Sync HEAD pointer is unreadable"))?;
        pointer
            .validate(&self.vault, &self.destination, device)
            .map_err(|_| corrupt("Durable Sync HEAD pointer failed authority validation"))?;
        Ok(Some((pointer, bytes)))
    }

    fn serialize(&self, record: &EventRecord) -> io::Result<Vec<u8>> {
        record.validate(&self.vault, &self.destination, &record.id)?;
        let bytes = serde_json::to_vec(record)
            .map_err(|_| invalid("Durable Sync event is not serializable"))?;
        if bytes.len() > MAX_EVENT_RECORD_BYTES {
            return Err(invalid("Durable Sync event exceeds its bound"));
        }
        Ok(bytes)
    }
}

fn compare_existing(record: EventRecord, existing: StoredEvent) -> io::Result<AcceptedEvent> {
    if existing != record.open()? {
        return Err(corrupt("Sync event retry does not match durable bytes"));
    }
    Ok(AcceptedEvent {
        event: existing,
        reused: true,
    })
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
        return Err(corrupt("Durable Sync record exceeds its bound"));
    }
    Ok(Some(bytes))
}

fn pointer_id(value: &(HeadPointer, Vec<u8>)) -> &str {
    &value.0.event_id
}

fn event_path(id: &str) -> String {
    format!("{id}.json")
}
fn pointer_path(device: &str) -> String {
    format!("{device}.json")
}
fn head_changed() -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, "Sync HEAD changed")
}
pub(super) fn corrupt(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
