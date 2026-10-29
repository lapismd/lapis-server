//! One vault owns IO and admitted work; clients only hold scoped attachments.
mod agent_bootstrap;
mod agent_context;
mod agent_skills;
mod backup;
mod byte_writes;
mod client;
pub mod daily;
mod database;
mod editing_intents;
mod editors;
mod error;
mod files;
mod guards;
mod history;
mod indexing;
mod operations;
mod plugins;
mod protocol;
mod runtime;
mod state;
mod streams;
mod tasks;
mod turns;
mod work;

pub use backup::{BackupStorage, HistoryIndexStatus};
pub use client::Client;
pub use database::DatabaseStorage;
pub use error::{Error, Result};
pub use history::{HistoryError, HistoryMethod};
pub use indexing::{
    IndexingStatus, SearchIndexingConfiguration, TrackedTaskError, TrackedTaskFailure,
    TrackedTaskProvider, TrackedTaskStatus,
};
pub use runtime::Runtime;
pub use state::VaultService;
pub use streams::FileSnapshot;
pub use tasks::{
    PreparedTaskLifecycle, TaskLifecycleDisposition, TaskLifecycleError, TaskLifecycleFailure,
    TaskLifecycleJournal, TaskLifecyclePartial, TaskLifecyclePolicy, TaskLifecycleRequest,
    TaskLifecycleStatus, TaskLifecycleWrite, TaskObservationWrite, TaskRecoveryError, TaskStorage,
    TaskWriteError, TaskWriteResult,
};
pub use turns::{
    AgentPolicy, ParticipantDelivery, ParticipantMessage, ParticipantMessageInput,
    ParticipantState, QueueEntry, QueueInput, QueuePhase, TurnQueue, TurnStorage,
};
pub use work::Work;
