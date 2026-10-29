// SPDX-License-Identifier: Apache-2.0
// Ported from Desktop's retained attachment preparation; reads are native ports.
use super::{AttachmentContext, AttachmentSource, ContextBlock, ContextKind};
use crate::services::{Error, Result};
use lapis_app_protocol::ErrorCode;
use serde_json::json;
use sha2::{Digest, Sha256};

pub(crate) const FILE_BYTES: usize = 65_536;
const MESSAGE_BYTES: usize = 131_072;
pub(crate) const LIMIT: &str = "File context exceeds 64 KiB per file or 128 KiB per message";

pub fn prepare_attachments(
    paths: &[String],
    mut read: impl FnMut(&str) -> Result<Option<String>>,
    cancelled: impl Fn() -> bool,
) -> Result<AttachmentContext> {
    let mut result = AttachmentContext {
        blocks: vec![],
        sources: vec![],
    };
    if paths.len() > 8 {
        return Err(invalid("Attach at most eight files per message"));
    }
    let mut bytes = 0usize;
    for path in paths {
        check_cancelled(&cancelled)?;
        if path.starts_with('/')
            || path.contains([':', '\\', '\0'])
            || path.split('/').any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(invalid("Attached files must use vault-relative paths"));
        }
        let text = read(path)?;
        check_cancelled(&cancelled)?;
        let text = text.ok_or_else(|| {
            Error::new(
                ErrorCode::NotFound,
                &format!("Attached file is unavailable: {path}"),
            )
        })?;
        bytes = bytes.saturating_add(text.len());
        if text.len() > FILE_BYTES || bytes > MESSAGE_BYTES {
            return Err(invalid(LIMIT));
        }
        result.sources.push(AttachmentSource {
            path: path.clone(),
            sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
            bytes: text.len(),
        });
        result.blocks.push(ContextBlock {
            kind: ContextKind::FileContext,
            id: format!("file:{path}"),
            content: format!("File: {path}\n\n{text}"),
            metadata: json!({ "path":path }),
        });
    }
    Ok(result)
}

fn check_cancelled(cancelled: &impl Fn() -> bool) -> Result<()> {
    if cancelled() {
        return Err(Error::new(ErrorCode::Closed, "This operation was aborted"));
    }
    Ok(())
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}
