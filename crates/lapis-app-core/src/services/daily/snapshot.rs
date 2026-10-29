use super::DailyDocumentCandidate;
use crate::{
    services::{
        Error, Result, Work, byte_writes,
        indexing::{manifest, records},
        state::State,
    },
    vault::VaultRoot,
};
use lapis_app_protocol::{ErrorCode, TextSnapshot, VaultPhase};
use std::collections::BTreeMap;

pub(super) struct Snapshot {
    files: Vec<String>,
    observed: BTreeMap<String, TextSnapshot>,
    pub candidates: Vec<DailyDocumentCandidate>,
}
pub(super) fn admitted_root(work: &Work) -> Result<VaultRoot> {
    let state = work.vault.lock()?;
    if state.phase == VaultPhase::Closed {
        return Err(Error::closed());
    }
    state.root.verify_physical_root()?;
    Ok(state.root.try_clone()?)
}
impl Snapshot {
    pub fn read(work: &Work) -> Result<Self> {
        let root = admitted_root(work)?;
        let files = manifest::collect(&root, || false)?.ok_or_else(Error::closed)?;
        let mut observed = BTreeMap::new();
        let mut candidates = Vec::new();
        let mut bytes = 0usize;
        for path in files.iter().filter(|path| records::markdown(path)) {
            let value = work.read_text(path)?;
            bytes = bytes.saturating_add(value.text.len());
            if bytes > 32 * 1024 * 1024 {
                return Err(Error::new(
                    ErrorCode::InvalidRequest,
                    "Native daily snapshot exceeds 32 MiB",
                ));
            }
            candidates.push(DailyDocumentCandidate {
                path: path.clone(),
                frontmatter:
                    lapis_plugin_services::markdown::extract_metadata(&value.text)["frontmatter"]
                        .clone(),
            });
            observed.insert(path.clone(), value);
        }
        Ok(Self {
            files,
            observed,
            candidates,
        })
    }
    pub fn verify(&self, state: &State) -> Result<()> {
        state.root.verify_physical_root()?;
        if manifest::collect(&state.root, || false)?.as_ref() != Some(&self.files) {
            return Err(Error::new(
                ErrorCode::Conflict,
                "Daily document inventory changed before use",
            ));
        }
        for (path, value) in &self.observed {
            byte_writes::read_versioned(state, path, &value.version)?;
        }
        state.root.verify_physical_root()?;
        Ok(())
    }
    pub fn existing(&self, path: &str) -> Result<TextSnapshot> {
        self.observed.get(path).cloned().ok_or_else(|| {
            Error::new(
                ErrorCode::PermissionDenied,
                "Daily document is outside the admitted snapshot",
            )
        })
    }
}
