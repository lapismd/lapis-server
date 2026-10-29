use super::{Client, Result, guards};
use crate::vault::WriteOptions;
use lapis_app_protocol::files::{EntryStat, FileCommand, FileListing, FileOptions, FileValue};

fn options(value: FileOptions) -> WriteOptions {
    WriteOptions { mtime: value.mtime }
}
impl Client {
    pub fn files(&self, command: FileCommand) -> Result<FileValue> {
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        state.root.verify_physical_root()?;
        let root = &state.root;
        // Keep write permission with the owning editor; structural changes require
        // all affected claims to have been saved or explicitly discarded first.
        match &command {
            FileCommand::CreateFile { path, .. }
            | FileCommand::WriteText { path, .. }
            | FileCommand::WriteTextIfUnchanged { path, .. }
            | FileCommand::WriteBinary { path, .. }
            | FileCommand::AppendText { path, .. } => {
                guards::write(&state, path, Some(self.id), false)?
            }
            FileCommand::Rename { path, new_path } => {
                guards::write(&state, path, None, true)?;
                guards::write(&state, new_path, None, true)?;
            }
            FileCommand::Remove { path } | FileCommand::Rmdir { path, .. } => {
                guards::write(&state, path, None, true)?
            }
            FileCommand::Copy { new_path, .. } | FileCommand::Mkdir { path: new_path, .. } => {
                guards::write(&state, new_path, None, false)?
            }
            _ => {}
        }
        Ok(match command {
            FileCommand::CreateFile {
                path,
                data,
                options: settings,
            } => {
                root.create_file(&path, data.as_slice(), &options(settings))?;
                FileValue::Done {}
            }
            FileCommand::Rename { path, new_path } => {
                root.rename(&path, &new_path)?;
                FileValue::Done {}
            }
            FileCommand::Copy { path, new_path } => {
                root.copy_file(&path, &new_path)?;
                FileValue::Done {}
            }
            FileCommand::Remove { path } => {
                root.remove_file(&path)?;
                FileValue::Done {}
            }
            FileCommand::Rmdir { path, recursive } => {
                root.remove_directory(&path, recursive)?;
                FileValue::Done {}
            }
            FileCommand::Stat { path } => FileValue::Stat {
                stat: root.stat(&path)?.map(|stat| EntryStat {
                    kind: stat.r#type.into(),
                    size: stat.size,
                    ctime: stat.ctime,
                    mtime: stat.mtime,
                }),
            },
            FileCommand::Exists { path } => FileValue::Exists {
                exists: root.stat(&path)?.is_some(),
            },
            FileCommand::List { path } => {
                let listing = root.list(&path)?;
                FileValue::List {
                    listing: FileListing {
                        files: listing.files,
                        folders: listing.folders,
                    },
                }
            }
            FileCommand::ReadText { path } => FileValue::Text {
                data: root.read_text(&path)?,
            },
            FileCommand::ReadBinary { path } => FileValue::Binary {
                data: root.read(&path)?,
            },
            FileCommand::ReadRange {
                path,
                offset,
                length,
            } => FileValue::Binary {
                data: root.read_range(&path, offset, length)?,
            },
            FileCommand::WriteText {
                path,
                data,
                options: settings,
            } => {
                root.write(&path, data.as_bytes(), &options(settings))?;
                FileValue::Done {}
            }
            FileCommand::WriteTextIfUnchanged {
                path,
                data,
                expected,
                options: settings,
            } => {
                root.write_if_unchanged(
                    &path,
                    data.as_bytes(),
                    expected.as_deref().map(str::as_bytes),
                    &options(settings),
                )?;
                FileValue::Done {}
            }
            FileCommand::WriteBinary {
                path,
                data,
                options: settings,
            } => {
                root.write(&path, &data, &options(settings))?;
                FileValue::Done {}
            }
            FileCommand::AppendText {
                path,
                data,
                options: settings,
            } => {
                root.append(&path, &data, &options(settings))?;
                FileValue::Done {}
            }
            FileCommand::Mkdir { path, recursive } => {
                root.mkdir(&path, recursive)?;
                FileValue::Done {}
            }
        })
    }
}
