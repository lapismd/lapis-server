//! Admitted Tasks commands. Native storage and effect authority are never values.
mod actions;
mod configuration;
mod observation;
mod status;
mod values;
use crate::TextSnapshot;
pub use actions::*;
pub use configuration::*;
pub use observation::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub use status::*;
use ts_rs::TS;
use uuid::Uuid;
pub use values::*;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum NativeTaskCommand {
    Configure {
        configuration: NativeTaskConfiguration,
    },
    Disable {},
    Status {},
    Mutate {
        path: String,
        expected_version: String,
        mutation: NativeTaskMutation,
    },
    Submit {
        request: NativeTaskLifecycleRequest,
    },
    Inspect {
        request_id: Uuid,
    },
    Retry {
        request_id: Uuid,
    },
    Occurrence {
        path: String,
        expected_version: String,
        date: String,
        completion: Option<NativeTaskCompletion>,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum NativeTaskValue {
    Status {
        status: Box<NativeTasksStatus>,
    },
    Snapshot {
        snapshot: TextSnapshot,
    },
    Operation {
        operation: Option<Box<NativeTaskOperation>>,
    },
    Observation {
        document: TextSnapshot,
        observation: Box<NativeTaskObservation>,
    },
    Failure {
        failure: Box<NativeTaskFailure>,
    },
}
