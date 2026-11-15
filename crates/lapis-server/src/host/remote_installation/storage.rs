use lapis_app_core::vault::{VaultRoot, WriteOptions};
use serde::{Deserialize, Serialize};
use std::{
    io,
    os::unix::fs::{MetadataExt, PermissionsExt},
};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Identity {
    pub installation: Uuid,
    pub owner: Uuid,
}
pub(super) fn child(parent: &VaultRoot, name: &str) -> io::Result<VaultRoot> {
    parent.verify_physical_root()?;
    let root = match parent.create_child(name) {
        Ok(root) => {
            root.open_directory("")?
                .open(".")?
                .into_std()
                .set_permissions(std::fs::Permissions::from_mode(0o700))?;
            root
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let witness = parent.open_directory(name)?.into_std_file().metadata()?;
            let expected = parent.path().join(name);
            let root = VaultRoot::open(&expected)?;
            if root.path() != expected
                || root.physical_identity()? != (witness.dev(), witness.ino())
            {
                return Err(invalid());
            }
            root
        }
        Err(error) => return Err(error),
    };
    if root
        .open_directory("")?
        .open(".")?
        .into_std()
        .metadata()?
        .permissions()
        .mode()
        & 0o077
        != 0
    {
        return Err(invalid());
    }
    parent.verify_physical_root()?;
    root.verify_physical_root()?;
    Ok(root)
}
pub(super) fn identity(root: &VaultRoot) -> io::Result<Identity> {
    if root.stat("identity.json")?.is_none() {
        // Never mint a new identity over existing ciphertext or saved selections.
        if root.directory_handle()?.entries()?.next().is_some() {
            return Err(invalid());
        }
        let identity = Identity {
            installation: Uuid::new_v4(),
            owner: Uuid::new_v4(),
        };
        let bytes = serde_json::to_vec(&identity).map_err(|_| invalid())?;
        match root.create_file("identity.json", &bytes[..], &WriteOptions::default()) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error),
        }
    }
    let metadata = root.entry_metadata("identity.json")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 256 {
        return Err(invalid());
    }
    let identity: Identity = serde_json::from_slice(&root.read_range("identity.json", 0, 257)?)
        .map_err(|_| invalid())?;
    if identity.installation.is_nil()
        || identity.owner.is_nil()
        || identity.owner == identity.installation
    {
        return Err(invalid());
    }
    Ok(identity)
}
fn invalid() -> io::Error {
    io::Error::other("Repository installation identity or private storage is unavailable")
}
