use super::*;
use serde::{Deserialize, Serialize};
const PATH: &str = "backup-synchronization.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Journal {
    version: u8,
    source: String,
    connection: RemoteConnection,
    pub proposal: Option<jj_vault::SyncProposal>,
    pub pending: Option<jj_vault::VaultRestoreRequest>,
    pub last_success: Option<i64>,
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
            if saved.pending.is_some() {
                return Err(Error::new(
                    ErrorCode::Conflict,
                    "Earlier synchronization needs explicit recovery",
                ));
            }
        }
        Ok(Self {
            version: 1,
            source: source.into(),
            connection,
            proposal: None,
            pending: None,
            last_success: None,
        })
    }
    pub fn save(&self, selection: &Selection) -> Result<()> {
        review_storage::save(selection, PATH, self)
    }
}
pub(super) fn pending(selection: &Selection) -> Result<bool> {
    Ok(
        review_storage::load::<Journal>(selection, PATH)?.is_some_and(|j| {
            j.pending.is_some() || j.proposal.is_some_and(|p| !p.conflicts.is_empty())
        }),
    )
}
