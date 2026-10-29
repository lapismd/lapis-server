use crate::services::{Error, Result, VaultService, guards, operations, state::State};
use lapis_app_protocol::{ErrorCode, ExpectedVersion, VaultPhase};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const PATH: &str = ".lapis/app.json";
pub(super) struct Configuration {
    pub enabled: bool,
    pub version: ExpectedVersion,
    value: Value,
}

impl Configuration {
    fn parse(text: Option<String>) -> Result<Self> {
        let version = match &text {
            Some(text) => ExpectedVersion::Version {
                version: format!("{:x}", Sha256::digest(text.as_bytes())),
            },
            None => ExpectedVersion::Missing {},
        };
        let value: Value = text.map_or_else(
            || Ok(json!({})),
            |text| serde_json::from_str(&text).map_err(|_| invalid()),
        )?;
        if !value.is_object()
            || value
                .get("pluginData")
                .is_some_and(|data| !data.is_object())
            || value
                .get("pluginData")
                .and_then(|data| data.get("repository-history"))
                .is_some_and(|data| !data.is_object())
        {
            return Err(invalid());
        }
        let enabled = value
            .get("pluginData")
            .and_then(|data| data.get("repository-history"))
            .and_then(|data| data.get("automaticBackup"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
        Ok(Self {
            enabled,
            version,
            value,
        })
    }
}

pub(super) fn read(vault: &VaultService) -> Result<Configuration> {
    access(vault, read_state)
}
pub(super) fn preflight(vault: &VaultService, expected: &ExpectedVersion) -> Result<()> {
    access(vault, |state| {
        guards::write(state, PATH, None, false)?;
        check(&read_state(state)?.version, expected)
    })
}
pub(super) fn set(
    vault: &VaultService,
    enabled: bool,
    expected: &ExpectedVersion,
) -> Result<Configuration> {
    access(vault, |state| {
        guards::write(state, PATH, None, false)?;
        let mut saved = read_state(state)?;
        check(&saved.version, expected)?;
        let root = saved.value.as_object_mut().ok_or_else(invalid)?;
        let data = root.entry("pluginData").or_insert_with(|| json!({}));
        let data = data.as_object_mut().ok_or_else(invalid)?;
        let repository = data
            .entry("repository-history")
            .or_insert_with(|| json!({}));
        let repository = repository.as_object_mut().ok_or_else(invalid)?;
        if repository.get("automaticBackup").and_then(Value::as_bool) == Some(enabled) {
            return Ok(saved);
        }
        repository.insert("automaticBackup".into(), Value::Bool(enabled));
        let text = serde_json::to_string_pretty(&saved.value).map_err(|_| invalid())?;
        state.root.mkdir(".lapis", true)?;
        operations::write(state, PATH, &text, expected, None)?;
        Configuration::parse(Some(text))
    })
}
fn access<T>(vault: &VaultService, action: impl FnOnce(&State) -> Result<T>) -> Result<T> {
    let storage = vault
        .backup_storage
        .as_ref()
        .ok_or_else(|| Error::new(ErrorCode::Unsupported, "Automatic backup is not selected"))?;
    let _files = storage
        .files
        .as_ref()
        .map(|files| files.lock())
        .transpose()
        .map_err(|_| super::failure())?;
    let state = vault.lock()?;
    if state.phase == VaultPhase::Closed {
        return Err(Error::closed());
    }
    state.root.verify_physical_root()?;
    storage.validate_vault(&state.root)?;
    action(&state)
}
fn read_state(state: &State) -> Result<Configuration> {
    let text = match state.root.read_text(PATH) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    Configuration::parse(text)
}
fn check(current: &ExpectedVersion, expected: &ExpectedVersion) -> Result<()> {
    let same = match (current, expected) {
        (ExpectedVersion::Missing {}, ExpectedVersion::Missing {}) => true,
        (ExpectedVersion::Version { version: a }, ExpectedVersion::Version { version: b }) => {
            a == b
        }
        _ => false,
    };
    if same {
        Ok(())
    } else {
        Err(Error::new(
            ErrorCode::Conflict,
            "Application configuration changed; review the saved setting again",
        ))
    }
}
fn invalid() -> Error {
    Error::new(
        ErrorCode::InvalidRequest,
        "Invalid saved application configuration",
    )
}
