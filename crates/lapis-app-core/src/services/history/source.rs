// Retained Desktop/hosted-services physical-source witnesses; Notes owns admission.
use std::os::unix::fs::MetadataExt;
use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Witness {
    path: PathBuf,
    identity: (u64, u64),
    directory: bool,
    contents: Option<Vec<u8>>,
}
impl Witness {
    fn capture(path: PathBuf, directory: bool) -> io::Result<Self> {
        let info = std::fs::symlink_metadata(&path)?;
        if info.file_type().is_symlink()
            || info.is_dir() != directory
            || (!directory && !info.is_file())
        {
            return Err(denied());
        }
        let contents = if directory {
            None
        } else {
            use std::io::Read;
            if info.len() > 4096 {
                return Err(denied());
            }
            let mut data = Vec::new();
            std::fs::File::open(&path)?
                .take(4097)
                .read_to_end(&mut data)?;
            if data.len() > 4096 {
                return Err(denied());
            }
            Some(data)
        };
        let witness = Self {
            path,
            identity: (info.dev(), info.ino()),
            directory,
            contents,
        };
        let after = std::fs::symlink_metadata(&witness.path)?;
        if after.file_type().is_symlink() || (after.dev(), after.ino()) != witness.identity {
            return Err(denied());
        }
        Ok(witness)
    }
    fn verify(&self) -> io::Result<()> {
        if Self::capture(self.path.clone(), self.directory)? != *self {
            return Err(denied());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Source {
    pub metadata: PathBuf,
    witnesses: Vec<Witness>,
}
impl Source {
    pub fn fingerprint(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new_with_prefix(b"lapis-history-source-witness-v1\0");
        for witness in &self.witnesses {
            let path = witness.path.as_os_str().as_encoded_bytes();
            hash.update((path.len() as u64).to_be_bytes());
            hash.update(path);
            hash.update(witness.identity.0.to_be_bytes());
            hash.update(witness.identity.1.to_be_bytes());
            hash.update([u8::from(witness.directory)]);
            if let Some(contents) = &witness.contents {
                hash.update((contents.len() as u64).to_be_bytes());
                hash.update(contents);
            }
        }
        hash.finalize().into()
    }
    /// Retained Desktop inspection; it neither initializes nor snapshots JJ.
    pub fn original(root: &Path) -> io::Result<Option<Self>> {
        match std::fs::symlink_metadata(root.join(".jj")) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        let jj_store_jj::local_repository::WorkspaceInfo {
            path,
            metadata,
            git,
            ..
        } = jj_store_jj::local_repository::inspect_workspace(root).map_err(|_| denied())?;
        if path != root {
            return Err(denied());
        }
        let mut witnesses = Vec::new();
        for directory in [
            path.clone(),
            path.join(".jj"),
            path.join(".jj/working_copy"),
            metadata.clone(),
            metadata.join("store"),
            git,
        ] {
            witnesses.push(Witness::capture(directory, true)?);
        }
        let link = path.join(".jj/repo");
        witnesses.push(Witness::capture(
            link.clone(),
            std::fs::symlink_metadata(&link)?.is_dir(),
        )?);
        for name in ["type", "git_target"] {
            witnesses.push(Witness::capture(metadata.join("store").join(name), false)?);
        }
        let source = Self {
            metadata,
            witnesses,
        };
        source.verify()?;
        Ok(Some(source))
    }
    /// Both paths come from the admitted native backup owner, never a read frame.
    pub fn existing(binding: &jj_vault::Binding, physical: (u64, u64)) -> io::Result<Option<Self>> {
        let state = &binding.state;
        if let Ok(info) = std::fs::symlink_metadata(state)
            && (!info.is_dir() || state.canonicalize()?.as_path() != state)
        {
            return Err(denied());
        }
        let markers = [
            state.join("vault.json"),
            state.join("repo"),
            state.join("working_copy"),
        ];
        if markers.iter().all(|path| {
            std::fs::symlink_metadata(path)
                .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
        }) {
            return Ok(None);
        }
        Self::external(binding, physical).map(Some)
    }
    fn external(binding: &jj_vault::Binding, physical: (u64, u64)) -> io::Result<Self> {
        let root = &binding.root;
        let state = &binding.state;
        let metadata = state.join("repo");
        let manifest = Witness::capture(state.join("vault.json"), false)?;
        let record: serde_json::Value =
            serde_json::from_slice(manifest.contents.as_deref().ok_or_else(denied)?)?;
        if record["version"].as_u64() != Some(1)
            || record["binding"] != serde_json::to_value(binding)?
            || record["root_identity"].as_str() != Some(&format!("{}:{}", physical.0, physical.1))
        {
            return Err(denied());
        }
        let mut witnesses = Vec::new();
        for path in [
            root.to_path_buf(),
            state.to_path_buf(),
            metadata.clone(),
            state.join("working_copy"),
            metadata.join("store"),
            metadata.join("store/git"),
        ] {
            witnesses.push(Witness::capture(path, true)?);
        }
        witnesses.push(manifest);
        for path in [
            state.join("working_copy/type"),
            metadata.join("store/type"),
            metadata.join("store/git_target"),
        ] {
            witnesses.push(Witness::capture(path, false)?);
        }
        let source = Self {
            metadata,
            witnesses,
        };
        source.verify()?;
        Ok(source)
    }
    pub fn verify(&self) -> io::Result<()> {
        for witness in &self.witnesses {
            witness.verify()?;
        }
        Ok(())
    }
}

fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Repository source changed or is unavailable",
    )
}
