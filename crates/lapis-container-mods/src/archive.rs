use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub const MAX_EXPANDED: u64 = 4 * 1024 * 1024 * 1024;
#[derive(Clone, Debug)]
pub enum Kind {
    Directory,
    File,
    Symlink(PathBuf),
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub kind: Kind,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
}
pub struct Plan {
    pub staged: tempfile::TempDir,
    pub entries: Vec<Entry>,
    pub replace: HashSet<PathBuf>,
    pub hooks: Vec<PathBuf>,
    pub ignored: HashSet<PathBuf>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    #[serde(default)]
    replace: Vec<String>,
}
pub fn relative(path: &Path) -> Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => (),
            _ => bail!("unsafe archive path"),
        }
    }
    ensure!(
        normalized.as_os_str().len() <= 4096,
        "archive path too long"
    );
    ensure!(
        !normalized
            .components()
            .any(|part| part.as_os_str().to_string_lossy().starts_with(".wh.")),
        "whiteouts are unsupported"
    );
    Ok(normalized)
}
fn reader(blob: &Path, media: &str) -> Result<Box<dyn Read>> {
    let file = File::open(blob)?;
    match media {
        "application/vnd.oci.image.layer.v1.tar" => Ok(Box::new(file)),
        "application/vnd.oci.image.layer.v1.tar+gzip"
        | "application/vnd.docker.image.rootfs.diff.tar.gzip" => {
            Ok(Box::new(flate2::read::GzDecoder::new(file)))
        }
        _ => bail!("unsupported Mod layer compression/media type"),
    }
}
pub fn prepare(blob: &Path, media: &str, workspace: &Path) -> Result<Plan> {
    let staged = tempfile::tempdir_in(workspace)?;
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut total = 0_u64;
    let mut archive = tar::Archive::new(reader(blob, media)?);
    for item in archive.entries()?.raw(true) {
        let mut item = item?;
        let path = relative(&item.path()?)?;
        if path.as_os_str().is_empty() {
            ensure!(
                item.header().entry_type().is_dir(),
                "empty archive filename"
            );
            continue;
        }
        ensure!(seen.insert(path.clone()), "duplicate archive path");
        ensure!(entries.len() < 100_000, "too many archive entries");
        let header = item.header();
        let mode = header.mode()?;
        ensure!(
            mode & 0o7000 == 0,
            "privileged archive permissions are unsupported"
        );
        let uid = u32::try_from(header.uid()?)?;
        let gid = u32::try_from(header.gid()?)?;
        total = total
            .checked_add(header.size()?)
            .context("archive size overflow")?;
        ensure!(total <= MAX_EXPANDED, "expanded Mod exceeds 4 GiB");
        let kind = match header.entry_type() {
            kind if kind.is_dir() => Kind::Directory,
            kind if kind.is_file() => Kind::File,
            kind if kind.is_symlink() => {
                let target = item
                    .link_name()?
                    .context("missing symlink target")?
                    .into_owned();
                ensure!(
                    !target.as_os_str().is_empty() && !target.is_absolute(),
                    "absolute symlinks are unsupported"
                );
                let mut depth = path.parent().unwrap().components().count();
                for component in target.components() {
                    match component {
                        Component::Normal(_) => depth += 1,
                        Component::CurDir => (),
                        Component::ParentDir if depth > 0 => depth -= 1,
                        _ => bail!("escaping symlink"),
                    }
                }
                Kind::Symlink(target)
            }
            _ => bail!("unsupported archive entry type"),
        };
        // Stage regular files under generated names, never under archive paths.
        if matches!(kind, Kind::File) {
            let mut output = File::create(staged.path().join(entries.len().to_string()))?;
            std::io::copy(&mut item, &mut output)?;
            output.flush()?;
        }
        entries.push(Entry {
            path,
            kind,
            mode,
            uid,
            gid,
        });
    }
    let mut trailer = Vec::new();
    archive
        .into_inner()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut trailer)?;
    ensure!(
        trailer.len() <= 1024 * 1024 && trailer.iter().all(|byte| *byte == 0),
        "invalid archive trailer"
    );
    ensure!(!entries.is_empty(), "empty Mod layer");
    let links: HashSet<_> = entries
        .iter()
        .filter(|entry| matches!(entry.kind, Kind::Symlink(_)))
        .map(|entry| &entry.path)
        .collect();
    for entry in &entries {
        for parent in entry.path.ancestors().skip(1) {
            ensure!(
                !links.contains(&parent.to_path_buf()),
                "archive writes through a symlink"
            );
        }
    }
    let mut replace = HashSet::new();
    let mut hooks = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let parts: Vec<_> = entry.path.components().collect();
        if parts.len() != 4 || parts[0].as_os_str() != "opt" || parts[1].as_os_str() != "mods" {
            continue;
        }
        let name = parts[3].as_os_str();
        if name == "mod.json" {
            ensure!(
                matches!(entry.kind, Kind::File)
                    && fs::metadata(staged.path().join(index.to_string()))?.len() <= 1024 * 1024,
                "invalid Mod metadata"
            );
            let metadata: Metadata =
                serde_json::from_reader(File::open(staged.path().join(index.to_string()))?)?;
            for path in metadata.replace {
                let path = relative(Path::new(&path))?;
                ensure!(!path.as_os_str().is_empty(), "empty replacement path");
                replace.insert(path);
            }
        } else if name == "init.sh" || name == "post-init.sh" {
            ensure!(
                matches!(entry.kind, Kind::File) && entry.mode & 0o111 != 0,
                "hooks must be executable regular files"
            );
            hooks.push(entry.path.clone());
        }
    }
    hooks.sort_by_key(|path| (path.file_name().unwrap() == "post-init.sh", path.clone()));
    Ok(Plan {
        staged,
        entries,
        replace,
        hooks,
        ignored: HashSet::new(),
    })
}
