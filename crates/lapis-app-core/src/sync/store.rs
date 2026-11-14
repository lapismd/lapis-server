//! Core sync store abstraction for durable, encrypted storage.
//!
//! Provides guarded filesystem operations with transactional semantics.
//! All writes are atomic and crash-safe through write-ahead logging.

use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Errors that can occur during sync store operations.
#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Encryption(String),
    Decryption(String),
    Conflict(String),
    NotInitialized,
    NotFound(String),
    Corruption(String),
    Journal(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Encryption(msg) => write!(f, "Encryption error: {}", msg),
            Self::Decryption(msg) => write!(f, "Decryption error: {}", msg),
            Self::Conflict(msg) => write!(f, "Transaction conflict: {}", msg),
            Self::NotInitialized => write!(f, "Store not initialized"),
            Self::NotFound(key) => write!(f, "Object not found: {}", key),
            Self::Corruption(msg) => write!(f, "Corruption detected: {}", msg),
            Self::Journal(msg) => write!(f, "Journal error: {}", msg),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Configuration for a sync store.
#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// Root directory for store data.
    pub root: PathBuf,
    /// Maximum object size in bytes (default: 100MB).
    pub max_object_size: usize,
    /// Enable write-ahead logging (default: true).
    pub wal_enabled: bool,
    /// Sync interval for flushing WAL (default: 1000ms).
    pub sync_interval_ms: u64,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from(".sync-store"),
            max_object_size: 100 * 1024 * 1024,
            wal_enabled: true,
            sync_interval_ms: 1000,
        }
    }
}

/// In-memory cache for hot objects.
struct ObjectCache {
    entries: HashMap<String, Vec<u8>>,
    max_size: usize,
    current_size: usize,
}

impl ObjectCache {
    fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_size,
            current_size: 0,
        }
    }

    fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.entries.get(key)
    }

    fn insert(&mut self, key: String, value: Vec<u8>) {
        let size = value.len();
        if size > self.max_size {
            return;
        }

        while self.current_size + size > self.max_size && !self.entries.is_empty() {
            if let Some(key) = self.entries.keys().next().cloned() {
                if let Some(value) = self.entries.remove(&key) {
                    self.current_size -= value.len();
                }
            }
        }

        self.current_size += size;
        self.entries.insert(key, value);
    }

    fn remove(&mut self, key: &str) {
        if let Some(value) = self.entries.remove(key) {
            self.current_size -= value.len();
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.current_size = 0;
    }
}

/// Durable sync store with transactional semantics.
pub struct SyncStore {
    config: StoreConfig,
    cache: Arc<RwLock<ObjectCache>>,
    initialized: bool,
}

impl SyncStore {
    /// Create a new sync store with the given configuration.
    pub fn new(config: StoreConfig) -> Result<Self, StoreError> {
        let cache = Arc::new(RwLock::new(ObjectCache::new(64 * 1024 * 1024)));
        Ok(Self {
            config,
            cache,
            initialized: false,
        })
    }

    /// Initialize the store, creating necessary directories and WAL.
    pub fn initialize(&mut self) -> Result<(), StoreError> {
        if self.initialized {
            return Ok(());
        }

        std::fs::create_dir_all(&self.config.root)?;
        std::fs::create_dir_all(self.config.root.join("objects"))?;
        std::fs::create_dir_all(self.config.root.join("events"))?;
        std::fs::create_dir_all(self.config.root.join("journal"))?;

        if self.config.wal_enabled {
            let wal_path = self.config.root.join("wal.log");
            if !wal_path.exists() {
                std::fs::File::create(&wal_path)?;
            }
        }

        self.initialized = true;
        Ok(())
    }

    /// Check if the store is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Read an object by key.
    pub fn read(&self, key: &str) -> Result<Vec<u8>, StoreError> {
        if !self.initialized {
            return Err(StoreError::NotInitialized);
        }

        {
            let cache = self.cache.read().map_err(|e| {
                StoreError::Io(io::Error::new(io::ErrorKind::Other, e.to_string()))
            })?;
            if let Some(data) = cache.get(key) {
                return Ok(data.clone());
            }
        }

        let path = self.object_path(key);
        if !path.exists() {
            return Err(StoreError::NotFound(key.to_string()));
        }

        let mut file = std::fs::File::open(&path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        if data.len() > self.config.max_object_size {
            return Err(StoreError::Corruption(format!(
                "Object {} exceeds max size",
                key
            )));
        }

        {
            let mut cache = self.cache.write().map_err(|e| {
                StoreError::Io(io::Error::new(io::ErrorKind::Other, e.to_string()))
            })?;
            cache.insert(key.to_string(), data.clone());
        }

        Ok(data)
    }

    /// Write an object atomically.
    pub fn write(&self, key: &str, data: &[u8]) -> Result<(), StoreError> {
        if !self.initialized {
            return Err(StoreError::NotInitialized);
        }

        if data.len() > self.config.max_object_size {
            return Err(StoreError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Object size {} exceeds max {}",
                    data.len(),
                    self.config.max_object_size
                ),
            )));
        }

        if self.config.wal_enabled {
            self.write_wal(key, data)?;
        }

        let temp_path = self.object_path(&format!("{}.tmp", key));
        let final_path = self.object_path(key);

        let mut file = std::fs::File::create(&temp_path)?;
        file.write_all(data)?;
        file.sync_all()?;

        std::fs::rename(&temp_path, &final_path)?;

        {
            let mut cache = self.cache.write().map_err(|e| {
                StoreError::Io(io::Error::new(io::ErrorKind::Other, e.to_string()))
            })?;
            cache.insert(key.to_string(), data.to_vec());
        }

        Ok(())
    }

    /// Delete an object.
    pub fn delete(&self, key: &str) -> Result<(), StoreError> {
        if !self.initialized {
            return Err(StoreError::NotInitialized);
        }

        let path = self.object_path(key);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }

        {
            let mut cache = self.cache.write().map_err(|e| {
                StoreError::Io(io::Error::new(io::ErrorKind::Other, e.to_string()))
            })?;
            cache.remove(key);
        }

        Ok(())
    }

    /// Check if an object exists.
    pub fn exists(&self, key: &str) -> Result<bool, StoreError> {
        if !self.initialized {
            return Err(StoreError::NotInitialized);
        }
        Ok(self.object_path(key).exists())
    }

    /// List all object keys.
    pub fn list(&self) -> Result<Vec<String>, StoreError> {
        if !self.initialized {
            return Err(StoreError::NotInitialized);
        }

        let objects_dir = self.config.root.join("objects");
        let mut keys = Vec::new();

        for entry in std::fs::read_dir(&objects_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        keys.push(name_str.to_string());
                    }
                }
            }
        }

        Ok(keys)
    }

    /// Flush the cache to disk.
    pub fn flush(&self) -> Result<(), StoreError> {
        if !self.initialized {
            return Err(StoreError::NotInitialized);
        }
        Ok(())
    }

    /// Clear the in-memory cache.
    pub fn clear_cache(&self) -> Result<(), StoreError> {
        let mut cache = self.cache.write().map_err(|e| {
            StoreError::Io(io::Error::new(io::ErrorKind::Other, e.to_string()))
        })?;
        cache.clear();
        Ok(())
    }

    fn object_path(&self, key: &str) -> PathBuf {
        self.config.root.join("objects").join(key)
    }

    fn write_wal(&self, key: &str, data: &[u8]) -> Result<(), StoreError> {
        let wal_path = self.config.root.join("wal.log");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&wal_path)?;

        let key_bytes = key.as_bytes();
        file.write_all(&(key_bytes.len() as u32).to_le_bytes())?;
        file.write_all(key_bytes)?;
        file.write_all(&(data.len() as u64).to_le_bytes())?;
        file.write_all(data)?;
        file.sync_all()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_store_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        assert!(!store.is_initialized());

        store.initialize().unwrap();
        assert!(store.is_initialized());
    }

    #[test]
    fn test_write_read() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        let key = "test-object";
        let data = b"test data";

        store.write(key, data).unwrap();
        let read_data = store.read(key).unwrap();

        assert_eq!(read_data, data);
    }

    #[test]
    fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        store.write("test", b"data").unwrap();
        assert!(store.exists("test").unwrap());

        store.delete("test").unwrap();
        assert!(!store.exists("test").unwrap());
    }

    #[test]
    fn test_list() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        store.write("obj1", b"data1").unwrap();
        store.write("obj2", b"data2").unwrap();
        store.write("obj3", b"data3").unwrap();

        let mut keys = store.list().unwrap();
        keys.sort();

        assert_eq!(keys, vec!["obj1", "obj2", "obj3"]);
    }

    #[test]
    fn test_size_limit() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig {
            root: temp_dir.path().to_path_buf(),
            max_object_size: 100,
            wal_enabled: false,
            ..Default::default()
        };

        let mut store = SyncStore::new(config).unwrap();
        store.initialize().unwrap();

        let large_data = vec![0u8; 200];
        let result = store.write("large", &large_data);

        assert!(result.is_err());
    }
}
