use lapis_app_core::{services::TaskStorage, vault::VaultRoot};
use std::{io, path::Path};

pub(in crate::host) fn select(profile: &Path) -> io::Result<TaskStorage> {
    let root = VaultRoot::open(profile)?;
    if let Err(error) = root.mkdir("tasks", false)
        && error.kind() != io::ErrorKind::AlreadyExists
    {
        return Err(error);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        root.open_directory("tasks")?
            .open(".")?
            .into_std()
            .set_permissions(std::fs::Permissions::from_mode(0o700))?;
    }
    root.verify_physical_root()?;
    TaskStorage::new(&root.path().join("tasks")).map_err(io::Error::other)
}
