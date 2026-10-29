//! Source-qualified daily policy; persistence stays with the admitted vault writer.
mod error;
mod format;
pub(in crate::services) mod io;
mod macros;
mod observation_error;
mod observations;
mod policy;
mod registry;
mod snapshot;
#[cfg(test)]
mod tests;
mod tokens;

pub use error::{DailyDocumentError, DailyDocumentOperationError, DailyDocumentOperationResult};
pub use format::{DailyFormatContext, format_daily_document_filename};
pub use observation_error::DefaultTaskObservationError;
pub use policy::{
    DailyDocumentCandidate, DailyDocumentPlan, DailyDocumentSettings, plan_daily_document,
};
pub use registry::{
    DailyDocumentProviderInfo, DailyDocumentProviderRegistry, DailyDocumentRegistration,
    DailyProviderChange,
};
