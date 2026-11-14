//! Event repository for durable storage of sync events.

use crate::sync::store::{StoreError, SyncStore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// Errors that can occur during repository operations.
#[derive(Debug)]
pub enum RepositoryError {
    Store(StoreError),
    NotFound(String),
    Invalid(String),
    Serialization(String),
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(e) => write!(f, "Store error: {}", e),
            Self::NotFound(id) => write!(f, "Event not found: {}", id),
            Self::Invalid(msg) => write!(f, "Invalid event: {}", msg),
            Self::Serialization(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for RepositoryError {}

impl From<StoreError> for RepositoryError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

/// Metadata for an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub event_id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub vault_id: Option<String>,
    pub stored_at: u64,
}

/// Filter for querying events.
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    pub ids: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub kinds: Option<Vec<u32>>,
    pub since: Option<u64>,
    pub until: Option<u64>,
    pub limit: Option<usize>,
}

impl EventFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ids(mut self, ids: Vec<String>) -> Self {
        self.ids = Some(ids);
        self
    }

    pub fn authors(mut self, authors: Vec<String>) -> Self {
        self.authors = Some(authors);
        self
    }

    pub fn kinds(mut self, kinds: Vec<u32>) -> Self {
        self.kinds = Some(kinds);
        self
    }

    pub fn since(mut self, since: u64) -> Self {
        self.since = Some(since);
        self
    }

    pub fn until(mut self, until: u64) -> Self {
        self.until = Some(until);
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Repository for managing sync events.
pub struct EventRepository {
    store: Arc<SyncStore>,
    metadata_index: std::sync::RwLock<HashMap<String, EventMetadata>>,
}

impl EventRepository {
    /// Create a new event repository backed by the given store.
    pub fn new(store: Arc<SyncStore>) -> Result<Self, RepositoryError> {
        let repo = Self {
            store,
            metadata_index: std::sync::RwLock::new(HashMap::new()),
        };
        repo.load_metadata()?;
        Ok(repo)
    }

    /// Store an event with its metadata.
    pub fn store_event(&self, event: &serde_json::Value) -> Result<String, RepositoryError> {
        let event_id = event
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RepositoryError::Invalid("Missing event ID".to_string()))?
            .to_string();

        let pubkey = event
            .get("pubkey")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RepositoryError::Invalid("Missing pubkey".to_string()))?
            .to_string();

        let created_at = event
            .get("created_at")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| RepositoryError::Invalid("Missing created_at".to_string()))?;

        let kind = event
            .get("kind")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| RepositoryError::Invalid("Missing kind".to_string()))?
            as u32;

        let tags: Vec<Vec<String>> = event
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|tag| {
                        tag.as_array().map(|t| {
                            t.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let vault_id = tags
            .iter()
            .find(|tag| tag.first().map(|s| s.as_str()) == Some("d"))
            .and_then(|tag| tag.get(1))
            .cloned();

        let metadata = EventMetadata {
            event_id: event_id.clone(),
            pubkey,
            created_at,
            kind,
            tags,
            vault_id,
            stored_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let event_bytes = serde_json::to_vec(event)
            .map_err(|e| RepositoryError::Serialization(e.to_string()))?;

        let key = format!("event_{}", event_id);
        self.store.write(&key, &event_bytes)?;

        let metadata_bytes = serde_json::to_vec(&metadata)
            .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
        let metadata_key = format!("meta_{}", event_id);
        self.store.write(&metadata_key, &metadata_bytes)?;

        {
            let mut index = self.metadata_index.write().map_err(|e| {
                RepositoryError::Store(StoreError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))
            })?;
            index.insert(event_id.clone(), metadata);
        }

        Ok(event_id)
    }

    /// Retrieve an event by ID.
    pub fn get_event(&self, event_id: &str) -> Result<serde_json::Value, RepositoryError> {
        let key = format!("event_{}", event_id);
        let data = self.store.read(&key).map_err(|e| match e {
            StoreError::NotFound(_) => RepositoryError::NotFound(event_id.to_string()),
            _ => RepositoryError::Store(e),
        })?;

        serde_json::from_slice(&data).map_err(|e| RepositoryError::Serialization(e.to_string()))
    }

    /// Get metadata for an event.
    pub fn get_metadata(&self, event_id: &str) -> Result<EventMetadata, RepositoryError> {
        let index = self.metadata_index.read().map_err(|e| {
            RepositoryError::Store(StoreError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        index
            .get(event_id)
            .cloned()
            .ok_or_else(|| RepositoryError::NotFound(event_id.to_string()))
    }

    /// Query events matching a filter.
    pub fn query(&self, filter: &EventFilter) -> Result<Vec<serde_json::Value>, RepositoryError> {
        let index = self.metadata_index.read().map_err(|e| {
            RepositoryError::Store(StoreError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        let mut matches: Vec<&EventMetadata> = index
            .values()
            .filter(|meta| {
                if let Some(ids) = &filter.ids {
                    if !ids.contains(&meta.event_id) {
                        return false;
                    }
                }
                if let Some(authors) = &filter.authors {
                    if !authors.contains(&meta.pubkey) {
                        return false;
                    }
                }
                if let Some(kinds) = &filter.kinds {
                    if !kinds.contains(&meta.kind) {
                        return false;
                    }
                }
                if let Some(since) = filter.since {
                    if meta.created_at < since {
                        return false;
                    }
                }
                if let Some(until) = filter.until {
                    if meta.created_at > until {
                        return false;
                    }
                }
                true
            })
            .collect();

        matches.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        if let Some(limit) = filter.limit {
            matches.truncate(limit);
        }

        let mut events = Vec::new();
        for meta in matches {
            match self.get_event(&meta.event_id) {
                Ok(event) => events.push(event),
                Err(RepositoryError::NotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }

        Ok(events)
    }

    /// Delete an event by ID.
    pub fn delete_event(&self, event_id: &str) -> Result<(), RepositoryError> {
        let event_key = format!("event_{}", event_id);
        self.store.delete(&event_key)?;

        let metadata_key = format!("meta_{}", event_id);
        self.store.delete(&metadata_key)?;

        {
            let mut index = self.metadata_index.write().map_err(|e| {
                RepositoryError::Store(StoreError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))
            })?;
            index.remove(event_id);
        }

        Ok(())
    }

    /// Count total events.
    pub fn count(&self) -> Result<usize, RepositoryError> {
        let index = self.metadata_index.read().map_err(|e| {
            RepositoryError::Store(StoreError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;
        Ok(index.len())
    }

    fn load_metadata(&self) -> Result<(), RepositoryError> {
        let keys = self.store.list()?;

        for key in keys {
            if !key.starts_with("meta_") {
                continue;
            }

            let data = self.store.read(&key)?;
            let metadata: EventMetadata = serde_json::from_slice(&data)
                .map_err(|e| RepositoryError::Serialization(e.to_string()))?;

            let mut index = self.metadata_index.write().map_err(|e| {
                RepositoryError::Store(StoreError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))
            })?;
            index.insert(metadata.event_id.clone(), metadata);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::store::StoreConfig;
    use serde_json::json;
    use tempfile::TempDir;

    fn create_test_event(id: &str, pubkey: &str, kind: u32, created_at: u64) -> serde_json::Value {
        json!({
            "id": id,
            "pubkey": pubkey,
            "created_at": created_at,
            "kind": kind,
            "tags": [],
            "content": "test content",
            "sig": "test sig"
        })
    }

    #[test]
    fn test_store_and_retrieve() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        let repo = EventRepository::new(Arc::new(store)).unwrap();

        let event = create_test_event("event1", "pubkey1", 1, 1000);
        let event_id = repo.store_event(&event).unwrap();

        assert_eq!(event_id, "event1");

        let retrieved = repo.get_event("event1").unwrap();
        assert_eq!(retrieved["id"], "event1");
        assert_eq!(retrieved["pubkey"], "pubkey1");
    }

    #[test]
    fn test_query_by_kind() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        let repo = EventRepository::new(Arc::new(store)).unwrap();

        repo.store_event(&create_test_event("e1", "pk1", 1, 1000))
            .unwrap();
        repo.store_event(&create_test_event("e2", "pk1", 2, 2000))
            .unwrap();
        repo.store_event(&create_test_event("e3", "pk1", 1, 3000))
            .unwrap();

        let filter = EventFilter::new().kinds(vec![1]);
        let results = repo.query(&filter).unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_delete_event() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        let repo = EventRepository::new(Arc::new(store)).unwrap();

        repo.store_event(&create_test_event("e1", "pk1", 1, 1000))
            .unwrap();
        assert_eq!(repo.count().unwrap(), 1);

        repo.delete_event("e1").unwrap();
        assert_eq!(repo.count().unwrap(), 0);
    }
}
