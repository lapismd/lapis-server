use lapis_app_core::vault::VaultRoot;
use std::{io, os::unix::fs::MetadataExt, path::Path};

pub(super) fn open(profile: &Path) -> io::Result<VaultRoot> {
    let profile = VaultRoot::open(profile)?;
    match profile.create_child("vaults") {
        Ok(root) => {
            private(&root)?;
            Ok(root)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => child(&profile, "vaults"),
        Err(error) => Err(error),
    }
}
pub(super) fn child(parent: &VaultRoot, name: &str) -> io::Result<VaultRoot> {
    parent.verify_physical_root()?;
    let witness = parent.open_directory(name)?.into_std_file().metadata()?;
    let expected = parent.path().join(name);
    let root = VaultRoot::open(&expected)?;
    if root.path() != expected || root.physical_identity()? != (witness.dev(), witness.ino()) {
        return Err(io::Error::other("Managed vault storage changed"));
    }
    parent.verify_physical_root()?;
    root.verify_physical_root()?;
    Ok(root)
}
pub(super) fn create(parent: &VaultRoot, id: uuid::Uuid) -> io::Result<VaultRoot> {
    parent.verify_physical_root()?;
    let container = parent.create_child(&id.to_string())?;
    private(&container)?;
    let content = container.create_child("content")?;
    private(&content)?;
    Ok(content)
}
fn private(root: &VaultRoot) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    root.open_directory("")?
        .open(".")?
        .into_std()
        .set_permissions(std::fs::Permissions::from_mode(0o700))
}
