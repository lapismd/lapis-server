use super::super::{manifest, records};
use crate::{
    services::{Error, Result},
    vault::VaultRoot,
};
use lapis_app_protocol::ErrorCode;
use lapis_task_services::{
    ParsedTaskDocument, ResolvedTaskDocumentStructure, TaskDocumentKind, TaskStructureOptions,
    TaskStructureTarget, parse_task_document, resolve_task_document_structure,
    validate_task_structure_graph,
};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) struct Snapshot {
    pub documents: Vec<ParsedTaskDocument>,
    pub structures: BTreeMap<String, ResolvedTaskDocumentStructure>,
    observed: BTreeMap<String, Vec<u8>>,
    bytes: usize,
}
impl Snapshot {
    fn read(&mut self, root: &VaultRoot, path: &str) -> Result<String> {
        if let Some(bytes) = self.observed.get(path) {
            return text(path, bytes);
        }
        root.verify_physical_root()?;
        let bytes = root.read(path)?;
        self.bytes = self.bytes.saturating_add(bytes.len());
        if self.bytes > 32 * 1024 * 1024 {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Native Task snapshot exceeds 32 MiB",
            ));
        }
        let decoded = text(path, &bytes)?;
        self.observed.insert(path.into(), bytes);
        Ok(decoded)
    }
    pub(super) fn prepare(root: &VaultRoot, files: &[String], stop: &AtomicBool) -> Result<Self> {
        let mut snapshot = Self {
            documents: vec![],
            structures: BTreeMap::new(),
            observed: BTreeMap::new(),
            bytes: 0,
        };
        for path in files.iter().filter(|path| records::markdown(path)) {
            if stop.load(Ordering::Acquire) {
                return Ok(snapshot);
            }
            let text = snapshot.read(root, path)?;
            if let Some(doc) = parse_task_document(&text, path) {
                snapshot.documents.push(doc);
            }
        }
        let mut structures = Vec::new();
        for doc in snapshot.documents.clone() {
            if doc.kind == TaskDocumentKind::TaskListNavigation {
                continue;
            }
            if stop.load(Ordering::Acquire) {
                return Ok(snapshot);
            }
            let options = TaskStructureOptions {
                document_id: doc.id,
                document_path: doc.path,
                document_kind: doc.kind,
            };
            let resolved = resolve_task_document_structure(
                &doc.raw,
                &options,
                |source: &str, href: &str| -> Result<Option<TaskStructureTarget>> {
                    let Some(path) = records::resolve(source, href, files) else {
                        return Ok(None);
                    };
                    let text = snapshot.read(root, &path)?;
                    Ok(Some(match parse_task_document(&text, &path) {
                        Some(target) if target.kind != TaskDocumentKind::TaskListNavigation => {
                            TaskStructureTarget {
                                id: target.id,
                                path: target.path,
                                title: target.title,
                                kind: if target.kind == TaskDocumentKind::Task {
                                    "task"
                                } else {
                                    "task-list"
                                }
                                .into(),
                            }
                        }
                        _ => unsupported(path),
                    }))
                },
            )?;
            structures.push(resolved);
        }
        snapshot.structures = validate_task_structure_graph(&structures)
            .into_iter()
            .collect();
        Ok(snapshot)
    }
    pub(super) fn unchanged(
        &self,
        root: &VaultRoot,
        files: &[String],
        stop: &AtomicBool,
    ) -> Result<bool> {
        if manifest::collect(root, || stop.load(Ordering::Acquire))?.as_deref() != Some(files) {
            return Ok(false);
        }
        for (path, bytes) in &self.observed {
            if stop.load(Ordering::Acquire) {
                return Ok(false);
            }
            root.verify_physical_root()?;
            match root.read(path) {
                Ok(current) if current == *bytes => {}
                Ok(_) => return Ok(false),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(true)
    }
}

fn text(path: &str, bytes: &[u8]) -> Result<String> {
    if records::markdown(path) {
        return String::from_utf8(bytes.to_vec())
            .map_err(|_| Error::new(ErrorCode::InvalidRequest, "Markdown source is not UTF-8"));
    }
    // The retained App resolver reads any admitted file as text. Keep binary
    // attachment targets unsupported while snapshot freshness uses original bytes.
    Ok(String::from_utf8_lossy(bytes).into_owned())
}
fn unsupported(path: String) -> TaskStructureTarget {
    let name = path.rsplit('/').next().unwrap_or(&path);
    TaskStructureTarget {
        id: path.clone(),
        title: name.rsplit_once('.').map_or(name, |(stem, _)| stem).into(),
        path,
        kind: "unsupported".into(),
    }
}
