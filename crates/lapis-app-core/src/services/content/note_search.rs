use super::{Result, failure};
use crate::services::Client;
use lapis_app_protocol::content::ContentErrorCode;
use serde_json::Value;

#[cfg(not(feature = "local-sibling-projections"))]
pub(super) fn search(_client: &Client, _input: &Value) -> Result<Value> {
    Err(failure(
        ContentErrorCode::Unsupported,
        "Native note search is unavailable",
    ))
}

#[cfg(feature = "local-sibling-projections")]
pub(super) fn search(client: &Client, input: &Value) -> Result<Value> {
    let runtime = tokio::runtime::Handle::try_current().map_err(|_| {
        failure(
            ContentErrorCode::Unavailable,
            "Search runtime is unavailable",
        )
    })?;
    runtime.block_on(native::search(client, input))
}

#[cfg(feature = "local-sibling-projections")]
mod native {
    use super::{Client, ContentErrorCode, Result, Value, failure};
    use crate::services::Error;
    use lapis_app_protocol::{
        ErrorCode,
        content::validate_content_path,
        database::search::retrieval::{SearchMode, SearchOptions, SearchRetrievalMode},
    };
    use serde_json::json;

    pub(super) async fn search(client: &Client, input: &Value) -> Result<Value> {
        let query = input["query"].as_str().unwrap_or_default().trim();
        if query.is_empty() || query.len() > 4096 {
            return Err(failure(
                ContentErrorCode::InvalidRequest,
                "Enter a search query up to 4096 bytes",
            ));
        }
        let folder = input["folder"].as_str();
        if let Some(folder) = folder {
            validate_content_path(folder)?;
            if folder.split('/').any(|part| part.starts_with('.')) {
                return Err(failure(
                    ContentErrorCode::InvalidRequest,
                    "Search folder must be public",
                ));
            }
        }
        let mode = match input["mode"].as_str().unwrap_or("auto") {
            "auto" => SearchMode::Auto,
            "keyword" => SearchMode::Lexical,
            "semantic" => SearchMode::Vector,
            "hybrid" => SearchMode::Hybrid,
            _ => {
                return Err(failure(
                    ContentErrorCode::InvalidRequest,
                    "Invalid search mode",
                ));
            }
        };
        client
            .vault
            .lock()
            .and_then(|state| state.require_client(client.id))
            .map_err(search_error)?;
        client
            .vault
            .ensure_content_search()
            .await
            .map_err(search_error)?;
        let database = client.vault.database().await.map_err(search_error)?;
        let index = client.vault.indexing_status().map_err(search_error)?;
        let index_status = if index.error.is_some() {
            "unavailable"
        } else if index.completed_at.is_none() || index.active || index.pending > 0 {
            "indexing"
        } else {
            "ready"
        };
        if index_status != "ready" && matches!(mode, SearchMode::Vector | SearchMode::Hybrid) {
            let error = if index_status == "indexing" {
                ContentErrorCode::Busy
            } else {
                ContentErrorCode::Unavailable
            };
            return Err(failure(error, "The vault search index is not ready"));
        }
        let result = database
            .search_notes(
                query,
                SearchOptions {
                    limit: Some(input["limit"].as_u64().unwrap_or(20).clamp(1, 50) as usize),
                    path_prefix: folder.map(str::to_owned),
                    snippet_length: Some(240),
                    mode: Some(if index_status != "ready" {
                        SearchMode::Lexical
                    } else {
                        mode
                    }),
                    ..SearchOptions::default()
                },
            )
            .await
            .map_err(search_error)?;
        let hits = result.rows.into_iter().filter_map(|row| {
            let path = row.document.path;
            super::super::note_writes::validate_note_path(&path).ok()?;
            // Do not return stale snippets after a note was changed, moved, or
            // replaced by a symlink while the index catches up.
            let current = client.read_text(&path).ok()?;
            if current.text != row.document.content {
                return None;
            }
            Some(json!({
                "path": path,
                "title": row.document.name,
                "score": row.score,
                "snippets": row.snippets.into_iter().take(2)
                    .map(|snippet| snippet.text.chars().take(240).collect::<String>()).collect::<Vec<_>>()
            }))
        }).collect::<Vec<_>>();
        let applied_mode = match result.mode {
            SearchRetrievalMode::Lexical => "keyword",
            SearchRetrievalMode::Vector => "semantic",
            SearchRetrievalMode::Hybrid => "hybrid",
        };
        Ok(json!({
            "mode": applied_mode, "index_status": index_status,
            "semantic_status": if index_status == "ready" { result.semantic_status } else { index_status },
            "hits": hits
        }))
    }

    fn search_error(error: Error) -> lapis_app_protocol::content::ContentError {
        let (code, message) = match error.code() {
            ErrorCode::Unsupported => (
                ContentErrorCode::Unsupported,
                "Enable native Markdown search and a local model in the vault's Search settings for semantic search",
            ),
            ErrorCode::Busy => (
                ContentErrorCode::Busy,
                "The vault search index or model is warming up",
            ),
            ErrorCode::PermissionDenied => (
                ContentErrorCode::PermissionDenied,
                "The vault is not admitted",
            ),
            _ => (
                ContentErrorCode::Unavailable,
                "The vault search index or local model is unavailable",
            ),
        };
        failure(code, message)
    }
}
