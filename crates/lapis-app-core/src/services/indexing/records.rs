use super::properties;
use crate::vault::FileStat;
use lapis_app_protocol::database::indexed::records::*;
use lapis_plugin_services::markdown::{PARSER_VERSION, content_hash, extract_metadata};
use serde_json::Value;
#[cfg(test)]
mod tests;

pub(in crate::services) fn markdown(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
    })
}
pub(super) fn record(
    path: &str,
    stat: &FileStat,
    text: &str,
    files: &[String],
    declared: &serde_json::Map<String, Value>,
) -> CoreIndexedFile {
    let metadata = extract_metadata(text);
    let hash = content_hash(text);
    let mut tags: Vec<TagRecord> = Vec::new();
    for value in metadata["tags"].as_array().into_iter().flatten() {
        if let Some(tag) = value["tag"].as_str() {
            add_tag(&mut tags, path, tag, Some(value["position"].clone()));
        }
    }
    for (name, value) in metadata["frontmatter"].as_object().into_iter().flatten() {
        if matches!(name.trim().to_lowercase().as_str(), "tag" | "tags") {
            if let Some(values) = value.as_array() {
                for value in values {
                    frontmatter_tag(&mut tags, path, value);
                }
            } else if let Some(value) = value.as_str() {
                for part in value.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';')) {
                    frontmatter_tag(&mut tags, path, &Value::String(part.into()));
                }
            }
        }
    }
    let properties = metadata["frontmatter"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(name, value)| PropertyRecord {
            path: path.into(),
            name: name.clone(),
            declared_type: declared
                .get(name)
                .and_then(Value::as_str)
                .map(str::to_owned),
            inferred_type: properties::inferred(name, value).into(),
            value: value.clone(),
        })
        .collect();
    let links = metadata["links"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(ordinal, value)| {
            let target = value["link"].as_str().unwrap_or_default();
            LinkRecord {
                source_path: path.into(),
                target_text: target.into(),
                original: value["original"].as_str().map(str::to_owned),
                resolved_target_path: resolve(path, target, files),
                link_type: LinkType::Link,
                position: Some(value["position"].clone()),
                count: 1.0,
                heading: Some(value["heading"].as_str().map(str::to_owned)),
                kind: Some(LinkKind::Reference),
                ordinal: Some(ordinal as f64),
            }
        })
        .collect();
    CoreIndexedFile {
        file: FileRecord {
            path: path.into(),
            normalized_path: path.into(),
            extension: path.rsplit('.').next().unwrap_or_default().to_lowercase(),
            mtime: stat.mtime,
            size: stat.size as f64,
            hash: hash.clone(),
            parser_version: None,
            indexed: true,
            deleted: None,
        },
        metadata: MetadataRecord {
            path: path.into(),
            hash,
            parser_version: PARSER_VERSION.into(),
            metadata,
        },
        links,
        tags,
        properties,
    }
}
fn add_tag(tags: &mut Vec<TagRecord>, path: &str, tag: &str, position: Option<Value>) {
    if let Some(existing) = tags.iter_mut().find(|row| row.tag == tag) {
        if position.is_some() {
            existing.position = position;
        }
        return;
    }
    let parts: Vec<String> = tag
        .trim_start_matches('#')
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();
    let hierarchy = (1..=parts.len())
        .map(|count| parts[..count].join("/"))
        .collect();
    tags.push(TagRecord {
        path: path.into(),
        tag: tag.into(),
        parts,
        hierarchy,
        position,
    });
}
fn frontmatter_tag(tags: &mut Vec<TagRecord>, path: &str, value: &Value) {
    if value.is_null() {
        return;
    }
    let value = value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string());
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    let tag = if value.starts_with('#') {
        value.to_owned()
    } else {
        format!("#{value}")
    };
    if tag[1..]
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | ' ' | '_' | '-'))
    {
        add_tag(tags, path, &tag, None);
    }
}
fn normalized(path: &str) -> Option<String> {
    if path.contains([':', '\\', '\0']) {
        return None;
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|part| *part != "..") => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}
pub(super) fn resolve(source: &str, target: &str, files: &[String]) -> Option<String> {
    let target = target.split(['|', '#']).next().unwrap_or_default();
    let normalized = target
        .replace('\\', "/")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_owned();
    if normalized.is_empty() {
        return None;
    }
    let relative = source.rsplit_once('/').map_or_else(
        || normalized.clone(),
        |(directory, _)| self::normalized(&format!("{directory}/{normalized}")).unwrap_or_default(),
    );
    for path in [&relative, &normalized] {
        for candidate in [
            path.clone(),
            format!("{path}.md"),
            format!("{path}.markdown"),
        ] {
            if files.contains(&candidate) {
                return Some(candidate);
            }
        }
    }
    let name = normalized.rsplit('/').next()?;
    let lowercase = name.to_lowercase();
    let basename = if lowercase.ends_with(".md") {
        &name[..name.len() - 3]
    } else if lowercase.ends_with(".markdown") {
        &name[..name.len() - 9]
    } else {
        name
    };
    files
        .iter()
        .find(|path| {
            markdown(path)
                && path
                    .rsplit('/')
                    .next()
                    .and_then(|name| name.rsplit_once('.'))
                    .is_some_and(|(stem, _)| stem == basename)
        })
        .or_else(|| {
            files.iter().find(|path| {
                [
                    format!("/{normalized}"),
                    format!("/{normalized}.md"),
                    format!("/{normalized}.markdown"),
                ]
                .iter()
                .any(|suffix| path.ends_with(suffix))
            })
        })
        .or_else(|| {
            files
                .iter()
                .find(|path| path.rsplit('/').next() == Some(name))
        })
        .cloned()
}
