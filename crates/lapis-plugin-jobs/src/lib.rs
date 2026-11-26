#![forbid(unsafe_code)]

#[cfg(feature = "engine")]
mod execute;
mod network;
mod schedule;
mod types;

#[cfg(feature = "engine")]
pub use execute::{ExecutionLimits, execute};
pub use network::*;
pub use schedule::{Occurrence, Schedule};
pub use types::*;

/// The owning native vault checks admission and permissions for every call.
pub trait JobHost: Send + Sync + 'static {
    fn invoke(&self, action: &str, input: serde_json::Value) -> Result<serde_json::Value, String>;
    fn cancelled(&self) -> bool;
    /// Native I/O shares the VM's terminal budget, including blocked reads.
    fn invoke_until(
        &self,
        action: &str,
        input: serde_json::Value,
        _deadline: std::time::Instant,
    ) -> Result<serde_json::Value, String> {
        self.invoke(action, input)
    }
}
