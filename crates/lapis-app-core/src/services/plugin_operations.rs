//! Trusted compiled providers. Transports never receive native clients.
use super::Client;
use lapis_app_protocol::{content::*, operations::*};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

type Result<T> = std::result::Result<T, ContentError>;
pub fn failure(code: ContentErrorCode, message: &str) -> ContentError {
    ContentError {
        code,
        message: message.into(),
        effect: ContentEffectStatus::NotStarted,
    }
}

pub trait OperationProvider: Send + Sync {
    fn descriptors(&self) -> Vec<OperationDescriptor>;
    fn available(&self, client: &Client) -> Result<bool>;
    /// Runs on the content boundary's retained blocking worker.
    fn invoke(&self, client: &Client, id: &str, request: &OperationRequest) -> Result<Value>;
}

struct Entry {
    descriptor: OperationDescriptor,
    digest: String,
    input: jsonschema::Validator,
    output: jsonschema::Validator,
    provider: Arc<dyn OperationProvider>,
}

#[derive(Default)]
pub struct OperationRegistry {
    entries: BTreeMap<String, Entry>,
}

pub fn operation_key(plugin: &str, id: &str) -> String {
    format!("{plugin}.{id}")
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
const RESERVED: &[&str] = &["help", "version", "capabilities", "vault", "note", "mcp"];

impl OperationRegistry {
    /// Registration is transactional: a bad provider publishes no partial catalog.
    pub fn register(&mut self, provider: Arc<dyn OperationProvider>) -> Result<()> {
        let descriptors = provider.descriptors();
        if descriptors.is_empty()
            || descriptors.len() > 128
            || self.entries.len() + descriptors.len() > 256
        {
            return Err(failure(
                ContentErrorCode::TooLarge,
                "Operation catalog exceeds bounds",
            ));
        }
        let mut pending = BTreeMap::new();
        for descriptor in descriptors {
            let key = operation_key(&descriptor.plugin_id, &descriptor.id);
            let command = &descriptor.help.command;
            let invalid = !identifier(&descriptor.plugin_id)
                || !identifier(&descriptor.id)
                || descriptor.version == 0
                || descriptor.description.trim().is_empty()
                || descriptor.description.len() > 4096
                || descriptor.help.markdown.len() > 32768
                || descriptor.help.summary.trim().is_empty()
                || !matches!(command.as_slice(), [root, action] if root == &descriptor.plugin_id && action == &descriptor.id)
                || RESERVED.contains(&descriptor.plugin_id.as_str())
                || !descriptor
                    .permission
                    .starts_with(&format!("{}:", descriptor.plugin_id))
                || !matches!(
                    descriptor.input_schema.get("type").and_then(Value::as_str),
                    Some("object")
                )
                || descriptor.input_schema.get("additionalProperties") != Some(&Value::Bool(false));
            if invalid || self.entries.contains_key(&key) || pending.contains_key(&key) {
                return Err(failure(
                    ContentErrorCode::InvalidRequest,
                    "Invalid or duplicate operation descriptor",
                ));
            }
            let mut flags = std::collections::BTreeSet::new();
            let mut fields = std::collections::BTreeSet::new();
            for arg in &descriptor.help.arguments {
                if !identifier(&arg.flag)
                    || [
                        "help",
                        "target",
                        "vault",
                        "token-file",
                        "token-stdin",
                        "json",
                        "timeout-ms",
                    ]
                    .contains(&arg.flag.as_str())
                    || !flags.insert(&arg.flag)
                    || !fields.insert(&arg.field)
                    || descriptor.input_schema["properties"]
                        .get(&arg.field)
                        .is_none()
                {
                    return Err(failure(
                        ContentErrorCode::InvalidRequest,
                        "Invalid operation CLI argument",
                    ));
                }
            }
            if descriptor.input_schema["required"]
                .as_array()
                .is_some_and(|required| {
                    required.iter().any(|name| {
                        !fields
                            .iter()
                            .any(|field| Some(field.as_str()) == name.as_str())
                    })
                })
            {
                return Err(failure(
                    ContentErrorCode::InvalidRequest,
                    "Required input has no CLI mapping",
                ));
            }
            let input = jsonschema::validator_for(&descriptor.input_schema).map_err(|_| {
                failure(
                    ContentErrorCode::InvalidRequest,
                    "Invalid operation input schema",
                )
            })?;
            let output = jsonschema::validator_for(&descriptor.output_schema).map_err(|_| {
                failure(
                    ContentErrorCode::InvalidRequest,
                    "Invalid operation output schema",
                )
            })?;
            let digest = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&descriptor).map_err(|_| failure(
                    ContentErrorCode::InvalidRequest,
                    "Invalid descriptor"
                ))?)
            );
            pending.insert(
                key,
                Entry {
                    descriptor,
                    digest,
                    input,
                    output,
                    provider: provider.clone(),
                },
            );
        }
        self.entries.extend(pending);
        Ok(())
    }

    pub fn snapshot(&self, permissions: &[String]) -> Result<BTreeMap<String, String>> {
        if permissions.len() > 128
            || permissions.iter().any(|permission| {
                !self
                    .entries
                    .values()
                    .any(|e| &e.descriptor.permission == permission)
            })
        {
            return Err(failure(
                ContentErrorCode::InvalidRequest,
                "Unknown operation permission",
            ));
        }
        Ok(self
            .entries
            .iter()
            .filter(|(_, entry)| permissions.contains(&entry.descriptor.permission))
            .map(|(key, entry)| (key.clone(), entry.digest.clone()))
            .collect())
    }

    pub fn catalog(
        &self,
        client: &Client,
        snapshot: &BTreeMap<String, String>,
    ) -> Result<Vec<OperationDescriptor>> {
        self.entries
            .iter()
            .filter(|(key, entry)| snapshot.get(*key) == Some(&entry.digest))
            .filter_map(|(_, entry)| match entry.provider.available(client) {
                Ok(true) => Some(Ok(entry.descriptor.clone())),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    pub fn invoke(
        &self,
        client: &Client,
        snapshot: &BTreeMap<String, String>,
        plugin: &str,
        id: &str,
        request: &OperationRequest,
    ) -> Result<Value> {
        let key = operation_key(plugin, id);
        let entry = self
            .entries
            .get(&key)
            .filter(|entry| snapshot.get(&key) == Some(&entry.digest))
            .ok_or_else(|| {
                failure(
                    ContentErrorCode::PermissionDenied,
                    "Operation is not admitted",
                )
            })?;
        if request.protocol_version != OPERATION_PROTOCOL_MAJOR
            || request.version != entry.descriptor.version
        {
            return Err(failure(
                ContentErrorCode::ProtocolMismatch,
                "Operation version mismatch",
            ));
        }
        if request.request_id.is_nil() || !entry.input.is_valid(&request.input) {
            return Err(failure(
                ContentErrorCode::InvalidRequest,
                "Invalid operation input",
            ));
        }
        if !entry.provider.available(client)? {
            return Err(failure(
                ContentErrorCode::Unavailable,
                "Plugin provider is disabled",
            ));
        }
        let value = entry.provider.invoke(client, id, request)?;
        if !entry.output.is_valid(&value) {
            return Err(ContentError {
                effect: if entry.descriptor.effect == ContentEffect::Write {
                    ContentEffectStatus::Unknown
                } else {
                    ContentEffectStatus::NotStarted
                },
                ..failure(
                    ContentErrorCode::OutcomeUnknown,
                    "Invalid provider result; inspect before retrying",
                )
            });
        }
        Ok(value)
    }
}
