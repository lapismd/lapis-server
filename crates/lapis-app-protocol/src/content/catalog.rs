use super::ContentCommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ContentOperation {
    Capabilities,
    ListVaults,
    ListNotes,
    ReadNote,
    CreateNote,
    ReplaceNote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ContentScope {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ContentEffect {
    ReadOnly,
    Write,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentOperationDescriptor {
    pub operation: ContentOperation,
    pub scope: ContentScope,
    pub effect: ContentEffect,
}

impl ContentOperation {
    pub const fn descriptor(self) -> ContentOperationDescriptor {
        let (scope, effect) = match self {
            Self::Capabilities | Self::ListVaults | Self::ListNotes | Self::ReadNote => {
                (ContentScope::Read, ContentEffect::ReadOnly)
            }
            Self::CreateNote | Self::ReplaceNote => (ContentScope::Write, ContentEffect::Write),
        };
        ContentOperationDescriptor {
            operation: self,
            scope,
            effect,
        }
    }
}

impl ContentCommand {
    pub const fn operation(&self) -> ContentOperation {
        match self {
            Self::Capabilities {} => ContentOperation::Capabilities,
            Self::ListVaults { .. } => ContentOperation::ListVaults,
            Self::ListNotes { .. } => ContentOperation::ListNotes,
            Self::ReadNote { .. } => ContentOperation::ReadNote,
            Self::CreateNote { .. } => ContentOperation::CreateNote,
            Self::ReplaceNote { .. } => ContentOperation::ReplaceNote,
        }
    }
}

/// Contract inventory only. A host must advertise only its qualified operations,
/// filtered through current grant/vault admission. This is not an allowlist.
pub const CONTENT_OPERATIONS: &[ContentOperationDescriptor] = &[
    ContentOperation::Capabilities.descriptor(),
    ContentOperation::ListVaults.descriptor(),
    ContentOperation::ListNotes.descriptor(),
    ContentOperation::ReadNote.descriptor(),
    ContentOperation::CreateNote.descriptor(),
    ContentOperation::ReplaceNote.descriptor(),
];
