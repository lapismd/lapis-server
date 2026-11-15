//! Native-private reviews are separate from path-free connection selection.
use super::*;
use crate::vault::{VaultRoot, WriteOptions};
use jj_store_jj::original_remotes::PushReview;
use serde::{Deserialize, Serialize};

const PATH: &str = "original-review.json";
const MAX: usize = 128 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Review {
    pub id: String,
    pub native: PushReview,
    // Set durably before execution. A failed response never grants replay.
    pub started: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Journal {
    version: u8,
    pub source: String,
    pub connection: RemoteConnection,
    pub observation: Option<RemoteObservation>,
    pub review: Option<Review>,
}
impl Journal {
    pub fn open(selection: &Selection, state: &SelectionState) -> Result<Option<Self>> {
        let root = VaultRoot::open(selection.private_path())?;
        root.verify_physical_root()?;
        let Some(metadata) = root.stat(PATH)? else {
            return Ok(None);
        };
        if metadata.r#type != "file" || metadata.size > MAX as u64 {
            return Err(invalid());
        }
        let bytes = root.read_range(PATH, 0, MAX + 1)?;
        if bytes.len() > MAX {
            return Err(invalid());
        }
        let saved: Self = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        if saved.version != 1 {
            return Err(invalid());
        }
        root.verify_physical_root()?;
        Ok((state.original.as_ref() == Some(&saved.connection)
            && state.original_binding.as_ref() == Some(&saved.source))
        .then_some(saved))
    }
    pub fn new(state: &SelectionState) -> Result<Self> {
        Ok(Self {
            version: 1,
            source: state.original_binding.clone().ok_or_else(invalid)?,
            connection: state.original.clone().ok_or_else(invalid)?,
            observation: None,
            review: None,
        })
    }
    pub fn save(&self, selection: &Selection) -> Result<()> {
        let root = VaultRoot::open(selection.private_path())?;
        root.verify_physical_root()?;
        let before = match root.stat(PATH)? {
            None => None,
            Some(metadata) if metadata.r#type == "file" && metadata.size <= MAX as u64 => {
                Some(root.read_range(PATH, 0, MAX + 1)?)
            }
            _ => return Err(invalid()),
        };
        let bytes = serde_json::to_vec(self).map_err(|_| invalid())?;
        if bytes.len() > MAX {
            return Err(invalid());
        }
        root.write_if_unchanged(
            PATH,
            &bytes[..],
            before.as_deref(),
            &WriteOptions::default(),
        )?;
        root.verify_physical_root()?;
        Ok(())
    }
}
pub(super) fn public(review: &Review) -> Result<RemotePushReview> {
    Ok(RemotePushReview {
        id: review.id.clone(),
        commit: review.native.commit().into(),
        summary: format!("Publish bookmark {}", review.native.bookmark()),
        outgoing: count(review.native.divergence().outgoing)?,
        remote_commit: review.native.divergence().remote_commit.clone(),
    })
}
pub(super) fn count(value: u64) -> Result<u32> {
    u32::try_from(value).map_err(|_| invalid())
}
fn invalid() -> Error {
    Error::new(ErrorCode::Io, "Saved repository review is unavailable")
}
