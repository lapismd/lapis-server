//! Apply journal for tracking durable apply operations.

use crate::sync::store::{StoreError, SyncStore};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// Errors that can occur during journal operations.
#[derive(Debug)]
pub enum JournalError {
    Store(StoreError),
    NotFound(String),
    Serialization(String),
    Duplicate(String),
}

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(e) => write!(f, "Store error: {}", e),
            Self::NotFound(id) => write!(f, "Entry not found: {}", id),
            Self::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            Self::Duplicate(id) => write!(f, "Duplicate entry: {}", id),
        }
    }
}

impl std::error::Error for JournalError {}

impl From<StoreError> for JournalError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

/// Type of apply operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationType {
    Create,
    Modify,
    Delete,
    Rename,
}

/// Status of an apply operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
}

/// Record of a single apply operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyEntry {
    pub receipt_id: String,
    pub revision_event_id: String,
    pub file_id: String,
    pub operation: OperationType,
    pub source_path: Option<String>,
    pub target_path: String,
    pub content_hash: Option<String>,
    pub status: OperationStatus,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub generation: u32,
}

/// Journal for tracking apply operations.
pub struct ApplyJournal {
    store: Arc<SyncStore>,
}

impl ApplyJournal {
    /// Create a new apply journal backed by the given store.
    pub fn new(store: Arc<SyncStore>) -> Self {
        Self { store }
    }

    /// Record the start of an apply operation.
    pub fn record_start(&self, entry: &ApplyEntry) -> Result<String, JournalError> {
        let key = format!("apply_{}", entry.receipt_id);
        if self.store.exists(&key)? {
            return Err(JournalError::Duplicate(entry.receipt_id.clone()));
        }

        let data = serde_json::to_vec(entry)
            .map_err(|e| JournalError::Serialization(e.to_string()))?;

        self.store.write(&key, &data)?;
        Ok(entry.receipt_id.clone())
    }

    /// Update the status of an apply operation.
    pub fn update_status(
        &self,
        receipt_id: &str,
        status: OperationStatus,
    ) -> Result<(), JournalError> {
        let key = format!("apply_{}", receipt_id);
        let data = self.store.read(&key).map_err(|e| match e {
            StoreError::NotFound(_) => JournalError::NotFound(receipt_id.to_string()),
            _ => JournalError::Store(e),
        })?;

        let mut entry: ApplyEntry = serde_json::from_slice(&data)
            .map_err(|e| JournalError::Serialization(e.to_string()))?;

        entry.status = status.clone();

        match status {
            OperationStatus::InProgress => {
                entry.started_at = Some(now_secs());
            }
            OperationStatus::Completed => {
                entry.completed_at = Some(now_secs());
            }
            _ => {}
        }

        let data = serde_json::to_vec(&entry)
            .map_err(|e| JournalError::Serialization(e.to_string()))?;

        self.store.write(&key, &data)?;
        Ok(())
    }

    /// Get an apply entry by receipt ID.
    pub fn get(&self, receipt_id: &str) -> Result<ApplyEntry, JournalError> {
        let key = format!("apply_{}", receipt_id);
        let data = self.store.read(&key).map_err(|e| match e {
            StoreError::NotFound(_) => JournalError::NotFound(receipt_id.to_string()),
            _ => JournalError::Store(e),
        })?;

        serde_json::from_slice(&data).map_err(|e| JournalError::Serialization(e.to_string()))
    }

    /// Check if a receipt exists.
    pub fn exists(&self, receipt_id: &str) -> Result<bool, JournalError> {
        let key = format!("apply_{}", receipt_id);
        Ok(self.store.exists(&key)?)
    }

    /// List all pending operations.
    pub fn list_pending(&self) -> Result<Vec<ApplyEntry>, JournalError> {
        let keys = self.store.list()?;
        let mut pending = Vec::new();

        for key in keys {
            if !key.starts_with("apply_") {
                continue;
            }

            let data = self.store.read(&key)?;
            let entry: ApplyEntry = serde_json::from_slice(&data)
                .map_err(|e| JournalError::Serialization(e.to_string()))?;

            if entry.status == OperationStatus::Pending
                || entry.status == OperationStatus::InProgress
            {
                pending.push(entry);
            }
        }

        Ok(pending)
    }

    /// List all operations for a revision.
    pub fn list_for_revision(
        &self,
        revision_event_id: &str,
    ) -> Result<Vec<ApplyEntry>, JournalError> {
        let keys = self.store.list()?;
        let mut entries = Vec::new();

        for key in keys {
            if !key.starts_with("apply_") {
                continue;
            }

            let data = self.store.read(&key)?;
            let entry: ApplyEntry = serde_json::from_slice(&data)
                .map_err(|e| JournalError::Serialization(e.to_string()))?;

            if entry.revision_event_id == revision_event_id {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Mark an operation as completed.
    pub fn mark_completed(&self, receipt_id: &str) -> Result<(), JournalError> {
        self.update_status(receipt_id, OperationStatus::Completed)
    }

    /// Mark an operation as failed.
    pub fn mark_failed(&self, receipt_id: &str, reason: String) -> Result<(), JournalError> {
        self.update_status(receipt_id, OperationStatus::Failed(reason))
    }

    /// Delete a completed entry.
    pub fn delete_completed(&self, receipt_id: &str) -> Result<(), JournalError> {
        let entry = self.get(receipt_id)?;

        if entry.status != OperationStatus::Completed {
            return Err(JournalError::Store(StoreError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot delete non-completed entry",
            ))));
        }

        let key = format!("apply_{}", receipt_id);
        self.store.delete(&key)?;
        Ok(())
    }

    /// Count total entries.
    pub fn count(&self) -> Result<usize, JournalError> {
        let keys = self.store.list()?;
        Ok(keys.iter().filter(|k| k.starts_with("apply_")).count())
    }

    /// Count pending entries.
    pub fn count_pending(&self) -> Result<usize, JournalError> {
        Ok(self.list_pending()?.len())
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::store::StoreConfig;
    use tempfile::TempDir;

    fn create_test_entry(receipt_id: &str, revision_id: &str, file_id: &str) -> ApplyEntry {
        ApplyEntry {
            receipt_id: receipt_id.to_string(),
            revision_event_id: revision_id.to_string(),
            file_id: file_id.to_string(),
            operation: OperationType::Create,
            source_path: None,
            target_path: "/test/path".to_string(),
            content_hash: Some("hash123".to_string()),
            status: OperationStatus::Pending,
            started_at: None,
            completed_at: None,
            generation: 1,
        }
    }

    #[test]
    fn test_record_and_retrieve() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        let journal = ApplyJournal::new(Arc::new(store));

        let entry = create_test_entry("r1", "rev1", "file1");
        let receipt_id = journal.record_start(&entry).unwrap();

        assert_eq!(receipt_id, "r1");

        let retrieved = journal.get("r1").unwrap();
        assert_eq!(retrieved.receipt_id, "r1");
        assert_eq!(retrieved.file_id, "file1");
    }

    #[test]
    fn test_update_status() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        let journal = ApplyJournal::new(Arc::new(store));

        let entry = create_test_entry("r1", "rev1", "file1");
        journal.record_start(&entry).unwrap();

        journal
            .update_status("r1", OperationStatus::InProgress)
            .unwrap();

        let updated = journal.get("r1").unwrap();
        assert_eq!(updated.status, OperationStatus::InProgress);
        assert!(updated.started_at.is_some());
    }

    #[test]
    fn test_idempotency() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        let journal = ApplyJournal::new(Arc::new(store));

        let entry = create_test_entry("r1", "rev1", "file1");
        journal.record_start(&entry).unwrap();

        let result = journal.record_start(&entry);
        assert!(matches!(result, Err(JournalError::Duplicate(_))));
    }

    #[test]
    fn test_list_pending() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        let journal = ApplyJournal::new(Arc::new(store));

        journal
            .record_start(&create_test_entry("r1", "rev1", "file1"))
            .unwrap();
        journal
            .record_start(&create_test_entry("r2", "rev1", "file2"))
            .unwrap();
        journal
            .record_start(&create_test_entry("r3", "rev1", "file3"))
            .unwrap();

        journal.mark_completed("r2").unwrap();

        let pending = journal.list_pending().unwrap();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_list_for_revision() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        let journal = ApplyJournal::new(Arc::new(store));

        journal
            .record_start(&create_test_entry("r1", "rev1", "file1"))
            .unwrap();
        journal
            .record_start(&create_test_entry("r2", "rev1", "file2"))
            .unwrap();
        journal
            .record_start(&create_test_entry("r3", "rev2", "file3"))
            .unwrap();

        let entries = journal.list_for_revision("rev1").unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_delete_completed() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        let journal = ApplyJournal::new(Arc::new(store));

        journal
            .record_start(&create_test_entry("r1", "rev1", "file1"))
            .unwrap();
        journal.mark_completed("r1").unwrap();

        journal.delete_completed("r1").unwrap();
        assert!(!journal.exists("r1").unwrap());
    }
}
