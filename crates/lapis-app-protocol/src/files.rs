//! Portable file actions used by existing shell adapters; OS actions stay in hosts.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct FileOptions {
    pub mtime: Option<f64>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(
    tag = "operation",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(
    tag = "operation",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum FileCommand {
    CreateFile {
        path: String,
        data: Vec<u8>,
        #[serde(default)]
        options: FileOptions,
    },
    Rename {
        path: String,
        new_path: String,
    },
    Copy {
        path: String,
        new_path: String,
    },
    Remove {
        path: String,
    },
    Rmdir {
        path: String,
        #[serde(default)]
        recursive: bool,
    },
    Stat {
        path: String,
    },
    Exists {
        path: String,
    },
    List {
        path: String,
    },
    ReadText {
        path: String,
    },
    ReadBinary {
        path: String,
    },
    ReadRange {
        path: String,
        #[ts(type = "number")]
        offset: u64,
        length: usize,
    },
    WriteText {
        path: String,
        data: String,
        #[serde(default)]
        options: FileOptions,
    },
    WriteTextIfUnchanged {
        path: String,
        data: String,
        expected: Option<String>,
        #[serde(default)]
        options: FileOptions,
    },
    WriteBinary {
        path: String,
        data: Vec<u8>,
        #[serde(default)]
        options: FileOptions,
    },
    AppendText {
        path: String,
        data: String,
        #[serde(default)]
        options: FileOptions,
    },
    Mkdir {
        path: String,
        #[serde(default)]
        recursive: bool,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct EntryStat {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub kind: String,
    #[ts(type = "number")]
    pub size: u64,
    pub ctime: f64,
    pub mtime: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct FileListing {
    pub files: Vec<String>,
    pub folders: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum FileValue {
    Text { data: String },
    Binary { data: Vec<u8> },
    Stat { stat: Option<EntryStat> },
    Exists { exists: bool },
    List { listing: FileListing },
    Done {},
}
