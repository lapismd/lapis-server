use anyhow::{Result, bail, ensure};
use oci_client::{Reference, secrets::RegistryAuth};
use std::{
    fs::OpenOptions,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

pub fn load(reference: &Reference) -> Result<RegistryAuth> {
    let directory = std::env::var_os("DOCKER_CONFIG")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".docker")));
    let Some(path) = directory
        .map(|path| path.join("config.json"))
        .filter(|path| path.exists())
    else {
        return Ok(RegistryAuth::Anonymous);
    };
    from_file(&path, reference)
}
pub fn from_file(path: &Path, reference: &Reference) -> Result<RegistryAuth> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let meta = file.metadata()?;
    // Root initialization must never execute helpers selected by app-writable data.
    ensure!(
        meta.is_file()
            && meta.uid() == unsafe { libc::geteuid() }
            && meta.mode() & 0o022 == 0
            && meta.len() <= 1024 * 1024,
        "registry config must be loader-owned and not group/world writable"
    );
    let value: serde_json::Value = serde_json::from_reader(file.try_clone()?)?;
    let server = if reference.registry() == "docker.io" {
        "https://index.docker.io/v1/"
    } else {
        reference.registry()
    };
    if let Some(helper) = value["credHelpers"][server]
        .as_str()
        .or_else(|| value["credsStore"].as_str())
    {
        ensure!(
            !helper.is_empty()
                && helper
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "invalid credential helper name"
        );
        for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
            ensure!(
                directory.is_absolute(),
                "credential helper PATH must be absolute"
            );
            let canonical = directory.canonicalize()?;
            for parent in canonical.ancestors() {
                let meta = std::fs::metadata(parent)?;
                ensure!(
                    meta.uid() == unsafe { libc::geteuid() } && meta.mode() & 0o022 == 0,
                    "credential helper PATH must not contain app-writable directories"
                );
            }
        }
        let name = format!("docker-credential-{helper}");
        let executable = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|part| part.join(&name))
            .find(|path| path.is_file());
        let path = executable
            .ok_or_else(|| anyhow::anyhow!("configured credential helper is unavailable"))?;
        let meta = std::fs::metadata(path)?;
        ensure!(
            meta.uid() == unsafe { libc::geteuid() } && meta.mode() & 0o022 == 0,
            "credential helper must not be app-writable"
        );
    }
    use std::io::{Seek, SeekFrom};
    let mut file = file;
    file.seek(SeekFrom::Start(0))?;
    match docker_credential::get_credential_from_reader(file, server) {
        Ok(docker_credential::DockerCredential::UsernamePassword(user, password)) => {
            Ok(RegistryAuth::Basic(user, password))
        }
        Ok(docker_credential::DockerCredential::IdentityToken(token)) => {
            Ok(RegistryAuth::Bearer(token))
        }
        Err(docker_credential::CredentialRetrievalError::NoCredentialConfigured) => {
            Ok(RegistryAuth::Anonymous)
        }
        Err(_) => bail!("configured registry credentials could not be loaded"),
    }
}
