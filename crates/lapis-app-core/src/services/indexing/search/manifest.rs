use crate::{
    database::Database,
    services::{Error, Result},
};
use lapis_app_protocol::{
    ErrorCode,
    database::search::{SearchManifestQuery, SearchManifestRecord},
};
use std::collections::BTreeMap;

pub(super) async fn collect(
    database: &Database,
    cancelled: impl Fn() -> bool,
) -> Result<Option<BTreeMap<String, SearchManifestRecord>>> {
    let mut result = BTreeMap::new();
    let mut after = None;
    let mut bytes = 0usize;
    loop {
        if cancelled() {
            return Ok(None);
        }
        let page = database
            .list_search_document_manifest(SearchManifestQuery {
                after,
                limit: Some(500),
            })
            .await?;
        for row in page.rows {
            bytes = bytes.saturating_add(serde_json::to_vec(&row).map_err(|_| bounds())?.len());
            if result.len() >= 100_000 || bytes > 32 * 1024 * 1024 {
                return Err(bounds());
            }
            result.insert(row.path.clone(), row);
        }
        after = page.next_cursor;
        if after.is_none() {
            return Ok(Some(result));
        }
    }
}
fn bounds() -> Error {
    Error::new(
        ErrorCode::InvalidRequest,
        "Native Search manifest exceeds bounds",
    )
}
