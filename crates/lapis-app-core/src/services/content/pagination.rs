use super::{Result, failure};
use lapis_app_protocol::content::{ContentErrorCode, ContentPageQuery};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub(super) enum Key {
    Vaults,
    Notes {
        vault: Uuid,
        directory: Option<String>,
    },
}

struct Cursor {
    id: String,
    key: Key,
    fingerprint: [u8; 32],
    offset: usize,
}

/// Retain at most 128 continuation handles. Evicted handles explicitly conflict;
/// no caller-chosen offset or state from another grant can select a page.
#[derive(Default)]
pub(super) struct Cursors(VecDeque<Cursor>);

impl Cursors {
    pub(super) fn page<T: Serialize>(
        &mut self,
        key: Key,
        items: Vec<T>,
        query: &ContentPageQuery,
    ) -> Result<(Vec<T>, Option<String>)> {
        let bytes = serde_json::to_vec(&items)
            .map_err(|_| failure(ContentErrorCode::Unavailable, "Listing unavailable"))?;
        let fingerprint: [u8; 32] = Sha256::digest(bytes).into();
        let offset = match &query.cursor {
            None => 0,
            Some(id) => self
                .0
                .iter()
                .find(|c| c.id == *id && c.key == key && c.fingerprint == fingerprint)
                .map(|c| c.offset)
                .ok_or_else(|| {
                    failure(
                        ContentErrorCode::Conflict,
                        "Listing or cursor changed; refresh the first page",
                    )
                })?,
        };
        let end = offset
            .saturating_add(usize::from(query.limit))
            .min(items.len());
        let next = if end < items.len() {
            if let Some(existing) = self
                .0
                .iter()
                .find(|c| c.key == key && c.fingerprint == fingerprint && c.offset == end)
            {
                Some(existing.id.clone())
            } else {
                let id = Uuid::new_v4().to_string();
                if self.0.len() == 128 {
                    self.0.pop_front();
                }
                self.0.push_back(Cursor {
                    id: id.clone(),
                    key,
                    fingerprint,
                    offset: end,
                });
                Some(id)
            }
        } else {
            None
        };
        Ok((
            items
                .into_iter()
                .skip(offset)
                .take(usize::from(query.limit))
                .collect(),
            next,
        ))
    }
}
