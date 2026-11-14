//! Sync storage layer for durable event and object repositories.
//!
//! This module provides guarded filesystem operations for sync stores,
//! including encrypted event/object persistence, apply journals, and
//! credential storage. All operations are transactional and crash-safe.

pub mod store;
pub mod repository;
pub mod journal;

pub use store::SyncStore;
pub use repository::EventRepository;
pub use journal::ApplyJournal;
