// SPDX-License-Identifier: Apache-2.0
use super::{BootstrapReader, FolderInstruction, cancelled, invalid, rules::has_host_path};
use crate::services::Result;

struct Pending {
    path: String,
    units: Vec<u16>,
    truncated: bool,
    omitted: Option<String>,
}

pub(super) fn collect(
    scope: &str,
    reader: &mut BootstrapReader<'_>,
    is_cancelled: &impl Fn() -> bool,
) -> Result<Vec<FolderInstruction>> {
    let segments = scope
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let mut collected = Vec::new();
    let mut remaining = 25_000usize;
    for index in 0..=segments.len() {
        let dir = segments[..index].join("/");
        let path = if dir.is_empty() {
            ".lapis/AGENTS.md".to_owned()
        } else {
            format!("{dir}/.lapis/AGENTS.md")
        };
        cancelled(is_cancelled)?;
        let raw = reader(&path)?;
        cancelled(is_cancelled)?;
        let Some(raw) = raw else { continue };
        let raw = raw.replace("\r\n", "\n");
        let text = crate::text::trim(&raw);
        if text.is_empty() {
            continue;
        }
        if has_host_path(&raw) {
            collected.push(Pending {
                path,
                units: Vec::new(),
                truncated: false,
                omitted: Some("path-bearing".into()),
            });
            continue;
        }
        let mut units = text.encode_utf16().collect::<Vec<_>>();
        let take = units.len().min(10_000).min(remaining);
        let truncated = take < units.len();
        units.truncate(take);
        remaining -= take;
        collected.push(Pending {
            path,
            units,
            truncated,
            omitted: None,
        });
        if remaining == 0 {
            break;
        }
    }
    // Preserve the entire source read trace before rejecting an undeliverable slice.
    collected
        .into_iter()
        .map(|entry| {
            Ok(FolderInstruction {
                path: entry.path,
                text: String::from_utf16(&entry.units)
                    .map_err(|_| invalid("Bootstrap instructions are not valid Unicode"))?,
                truncated: entry.truncated,
                omitted: entry.omitted,
            })
        })
        .collect()
}
