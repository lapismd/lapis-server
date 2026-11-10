use crate::archive::{Entry, Kind};
use anyhow::{Result, bail, ensure};
use cap_std::fs::Dir;
use std::{
    collections::{HashMap, VecDeque},
    path::{Component, Path},
};

#[derive(Clone)]
enum Part {
    Name(std::ffi::OsString),
    Up,
}
fn parts(path: &Path) -> Result<VecDeque<Part>> {
    path.components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(Ok(Part::Name(name.to_owned()))),
            Component::ParentDir => Some(Ok(Part::Up)),
            Component::CurDir => None,
            _ => Some(Err(anyhow::anyhow!("absolute/escaping symlink target"))),
        })
        .collect()
}
/// Resolve the graph rather than lexically collapsing `link/..` before links.
/// Planned paths shadow old filesystem paths; capability access bounds fallback.
pub fn validate<'a>(root: &Dir, entries: impl IntoIterator<Item = &'a Entry>) -> Result<()> {
    let entries: Vec<_> = entries.into_iter().collect();
    let planned: HashMap<_, _> = entries
        .iter()
        .map(|entry| (entry.path.clone(), &entry.kind))
        .collect();
    for entry in entries {
        let Kind::Symlink(target) = &entry.kind else {
            continue;
        };
        let mut location = entry.path.parent().unwrap().to_path_buf();
        let mut pending = parts(target)?;
        let mut hops = 0;
        while let Some(part) = pending.pop_front() {
            match part {
                Part::Up => ensure!(location.pop(), "symlink graph escapes the admitted root"),
                Part::Name(name) => {
                    location.push(name);
                    let link = match planned.get(&location) {
                        Some(Kind::Symlink(target)) => Some(target.clone()),
                        Some(Kind::Directory) => None,
                        Some(Kind::File) => {
                            ensure!(pending.is_empty(), "symlink target traverses a file");
                            None
                        }
                        None => match root.symlink_metadata(&location) {
                            Ok(meta) if meta.file_type().is_symlink() => {
                                Some(root.read_link(&location)?)
                            }
                            Ok(meta) => {
                                ensure!(
                                    pending.is_empty() || meta.is_dir(),
                                    "symlink target traverses a file"
                                );
                                None
                            }
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                            Err(error) => return Err(error.into()),
                        },
                    };
                    if let Some(target) = link {
                        hops += 1;
                        ensure!(hops <= 40, "symlink loop or resolution limit");
                        location.pop();
                        let mut replacement = parts(&target)?;
                        replacement.append(&mut pending);
                        pending = replacement;
                        if pending.len() > 4096 {
                            bail!("symlink expansion limit");
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
