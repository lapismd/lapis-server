//! Backup reviews retain an immutable native intent independently of new captures.
use super::*;
use serde::{Deserialize, Serialize};
const PATH: &str = "backup-review.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Review {
    pub native: jj_vault::PushIntent,
    pub counts: jj_store_jj::vault_workspace::CommitDivergence,
    pub started: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Journal {
    version: u8,
    source: String,
    connection: RemoteConnection,
    pub observation: Option<RemoteObservation>,
    pub review: Option<Review>,
}
impl Journal {
    pub fn open(selection: &Selection, state: &SelectionState, source: &str) -> Result<Self> {
        let connection = state.backup.clone().ok_or_else(unsupported)?;
        if let Some(saved) = review_storage::load::<Self>(selection, PATH)? {
            if saved.version != 1 {
                return Err(review_storage::invalid());
            }
            if saved.source == source && saved.connection == connection {
                return Ok(saved);
            }
        }
        Ok(Self {
            version: 1,
            source: source.into(),
            connection,
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
        id: review.native.request_id().into(),
        commit: review.native.commit().into(),
        summary: "Publish the reviewed local vault checkpoint".into(),
        outgoing: journal::count(review.counts.outgoing)?,
        remote_commit: review.native.remote_commit().map(str::to_owned),
    })
}
