use crate::archive::{Kind, Plan};
use anyhow::{Result, ensure};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use std::{
    ffi::CString,
    fs::File,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

fn parents(root: &Dir, path: &Path) -> Result<()> {
    let mut cursor = PathBuf::new();
    for component in path.parent().unwrap_or(Path::new("")).components() {
        cursor.push(component);
        match root.symlink_metadata(&cursor) {
            Ok(meta) => ensure!(
                meta.is_dir() && !meta.file_type().is_symlink(),
                "Mod parent is not a real directory"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
pub fn preflight(root: &Dir, plan: &Plan) -> Result<()> {
    crate::links::validate(
        root,
        plan.entries
            .iter()
            .filter(|entry| !plan.ignored.contains(&entry.path)),
    )?;
    for entry in plan
        .entries
        .iter()
        .filter(|entry| !plan.ignored.contains(&entry.path))
    {
        let first = entry.path.components().next().unwrap().as_os_str();
        ensure!(
            !["proc", "sys", "dev", "run"]
                .iter()
                .any(|name| first == *name),
            "Mod cannot modify runtime mounts"
        );
        ensure!(
            entry.path != Path::new("usr/local/bin/mod-loader")
                && !entry.path.starts_with("var/lib/container-mods"),
            "Mod cannot replace loader state"
        );
        parents(root, &entry.path)?;
        match root.symlink_metadata(&entry.path) {
            Ok(meta) if matches!(entry.kind, Kind::Directory) => ensure!(
                meta.is_dir() && !meta.file_type().is_symlink(),
                "directory collision"
            ),
            Ok(meta) => ensure!(
                !meta.is_dir() && plan.replace.contains(&entry.path),
                "existing file requires explicit replacement: {}",
                entry.path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
fn own(fd: i32, uid: u32, gid: u32) -> Result<()> {
    // SAFETY: fchown operates only on the already opened capability-bound fd.
    if unsafe { libc::geteuid() } == 0 {
        ensure!(
            unsafe { libc::fchown(fd, uid, gid) } == 0,
            "cannot preserve Mod ownership"
        );
    } else {
        ensure!(
            uid == unsafe { libc::geteuid() } && gid == unsafe { libc::getegid() },
            "Mod ownership requires root"
        );
    }
    Ok(())
}
struct Change {
    path: PathBuf,
    backup: Option<PathBuf>,
}
pub fn apply(root: &Dir, plan: &Plan) -> Result<()> {
    preflight(root, plan)?;
    let mut journal: Vec<Change> = Vec::new();
    let nonce = plan.staged.path().file_name().unwrap().to_string_lossy();
    let result = (|| -> Result<()> {
        for (index, entry) in plan
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| !plan.ignored.contains(&entry.path))
        {
            parents(root, &entry.path)?;
            if matches!(entry.kind, Kind::Directory) {
                if root.symlink_metadata(&entry.path).is_err() {
                    root.create_dir_all(&entry.path)?;
                    let directory = root.open_dir(&entry.path)?;
                    own(directory.as_raw_fd(), entry.uid, entry.gid)?;
                    root.set_permissions(
                        &entry.path,
                        cap_std::fs::Permissions::from_std(std::fs::Permissions::from_mode(
                            entry.mode,
                        )),
                    )?;
                }
                continue;
            }
            let parent = entry
                .path
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            root.create_dir_all(parent)?;
            let temp = parent.join(format!(".mod-new-{nonce}-{index}"));
            let backup = parent.join(format!(".mod-old-{nonce}-{index}"));
            ensure!(
                root.symlink_metadata(&temp).is_err() && root.symlink_metadata(&backup).is_err(),
                "Mod transaction filename already exists"
            );
            match &entry.kind {
                Kind::File => {
                    let mut output = root
                        .open_with(
                            &temp,
                            OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .follow(FollowSymlinks::No),
                        )?
                        .into_std();
                    let write = (|| -> Result<()> {
                        std::io::copy(
                            &mut File::open(plan.staged.path().join(index.to_string()))?,
                            &mut output,
                        )?;
                        own(output.as_raw_fd(), entry.uid, entry.gid)?;
                        output.set_permissions(std::fs::Permissions::from_mode(entry.mode))?;
                        output.sync_all()?;
                        Ok(())
                    })();
                    if let Err(error) = write {
                        let _ = root.remove_file(&temp);
                        return Err(error);
                    }
                }
                Kind::Symlink(target) => {
                    root.symlink(target, &temp)?;
                    let parent_dir = root.open_dir(parent)?;
                    let name = CString::new(temp.file_name().unwrap().as_encoded_bytes())?;
                    // SAFETY: the parent fd is capability-bound; name has one component.
                    if unsafe { libc::geteuid() } == 0
                        && unsafe {
                            libc::fchownat(
                                parent_dir.as_raw_fd(),
                                name.as_ptr(),
                                entry.uid,
                                entry.gid,
                                libc::AT_SYMLINK_NOFOLLOW,
                            )
                        } != 0
                    {
                        let _ = root.remove_file(&temp);
                        return Err(std::io::Error::last_os_error().into());
                    }
                }
                Kind::Directory => unreachable!(),
            }
            let previous = if root.symlink_metadata(&entry.path).is_ok() {
                ensure!(
                    plan.replace.contains(&entry.path),
                    "concurrent file collision"
                );
                // Preserve the old inode without removing the live name before publication.
                root.hard_link(&entry.path, root, &backup)?;
                Some(backup)
            } else {
                None
            };
            journal.push(Change {
                path: entry.path.clone(),
                backup: previous,
            });
            if let Err(error) = root.rename(&temp, root, &entry.path) {
                let _ = root.remove_file(&temp);
                return Err(error.into());
            }
        }
        Ok(())
    })();
    if result.is_err() {
        for change in journal.iter().rev() {
            let _ = root.remove_file(&change.path);
            if let Some(backup) = &change.backup {
                root.rename(backup, root, &change.path)?;
            }
        }
    } else {
        for change in journal {
            if let Some(backup) = change.backup {
                root.remove_file(backup)?;
            }
        }
    }
    result
}

/// A receipt is an optimization only; filesystem bytes and kinds still agree.
pub fn matches(root: &Dir, plan: &Plan) -> bool {
    plan.entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| !plan.ignored.contains(&entry.path))
        .all(|(index, entry)| {
            let result = (|| -> Result<bool> {
                parents(root, &entry.path)?;
                let meta = root.symlink_metadata(&entry.path)?;
                Ok(match &entry.kind {
                    Kind::Directory => meta.is_dir() && !meta.file_type().is_symlink(),
                    Kind::Symlink(target) => {
                        meta.file_type().is_symlink() && root.read_link(&entry.path)? == *target
                    }
                    Kind::File => {
                        if !meta.is_file() || meta.file_type().is_symlink() {
                            return Ok(false);
                        }
                        let file = root
                            .open_with(
                                &entry.path,
                                OpenOptions::new().read(true).follow(FollowSymlinks::No),
                            )?
                            .into_std();
                        let metadata = file.metadata()?;
                        metadata.uid() == entry.uid
                            && metadata.gid() == entry.gid
                            && metadata.permissions().mode() & 0o777 == entry.mode
                            && file_hash(file)?
                                == file_hash(File::open(
                                    plan.staged.path().join(index.to_string()),
                                )?)?
                    }
                })
            })();
            result.unwrap_or(false)
        })
}
fn file_hash(mut file: File) -> Result<Vec<u8>> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut hash = Sha256::new();
    let mut bytes = [0; 65536];
    loop {
        let count = file.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        hash.update(&bytes[..count]);
    }
    Ok(hash.finalize().to_vec())
}
