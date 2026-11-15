//! Native-private reviews are separate from path-free connection selection.
use super::*;
use jj_store_jj::original_remotes::PushReview;
use serde::{Deserialize, Serialize};

const PATH: &str = "original-review.json";
pub(super) fn held_credential(root: &crate::vault::VaultRoot) -> Result<Option<String>> {
    let saved: Option<Journal> = review_storage::load_root(root, PATH)?;
    Ok(saved
        .filter(|j| j.review.as_ref().is_some_and(|r| r.started))
        .map(|j| j.connection.credential)
        .filter(|id| !id.is_empty()))
}
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
        let Some(saved): Option<Self> = review_storage::load(selection, PATH)? else {
            return Ok(None);
        };
        if saved.version != 1 {
            return Err(invalid());
        }
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
        review_storage::save(selection, PATH, self)
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
