use super::{Control, now_ms, passwords};
use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Installation-only input. No HTTP request can select these native paths.
pub struct Bootstrap {
    pub username: String,
    pub password_file: Option<PathBuf>,
}

pub(crate) fn select_owner_password(
    environment: Option<&str>,
    file_contents: Option<&str>,
) -> io::Result<String> {
    if let Some(password) = environment
        .map(str::trim)
        .filter(|password| !password.is_empty())
    {
        return Ok(password.to_owned());
    }
    let contents = file_contents.ok_or_else(|| {
        io::Error::other(
            "Bootstrap password file is required when LAPIS_SERVER_BOOTSTRAP_PASSWORD is unset",
        )
    })?;
    Ok(contents.trim_end_matches(['\r', '\n']).to_owned())
}

pub(crate) fn password_source_message(environment: Option<&str>) -> &'static str {
    if environment
        .map(str::trim)
        .is_some_and(|password| !password.is_empty())
    {
        "owner password source: environment"
    } else {
        "owner password source: file"
    }
}
pub async fn open(
    profile: &Path,
    vaults: &[PathBuf],
    bootstrap: Option<Bootstrap>,
) -> io::Result<Arc<Control>> {
    if !profile.is_absolute() {
        return Err(io::Error::other("Authentication profile must be absolute"));
    }
    std::fs::create_dir_all(profile)?;
    let profile = profile.canonicalize()?;
    let roots: Vec<_> = vaults
        .iter()
        .map(|root| root.canonicalize())
        .collect::<io::Result<_>>()?;
    if roots
        .iter()
        .any(|root| profile.join("control").starts_with(root))
    {
        return Err(io::Error::other(
            "Authentication control data must stay outside admitted vaults",
        ));
    }
    let control = Arc::new(Control::open(&profile).await?);
    if control.initialized().await? {
        return Ok(control);
    }
    let bootstrap = bootstrap.ok_or_else(|| {
        io::Error::other("Bootstrap username and password file are required when no owner exists")
    })?;
    let environment = std::env::var("LAPIS_SERVER_BOOTSTRAP_PASSWORD").ok();
    let file_contents = if password_source_message(environment.as_deref())
        == "owner password source: environment"
    {
        None
    } else {
        let password_file = bootstrap.password_file.as_ref().ok_or_else(|| {
            io::Error::other(
                "Bootstrap password file is required when LAPIS_SERVER_BOOTSTRAP_PASSWORD is unset",
            )
        })?;
        let password_file = password_file.canonicalize()?;
        if roots.iter().any(|root| password_file.starts_with(root)) {
            return Err(io::Error::other(
                "Bootstrap password file must stay outside admitted vaults",
            ));
        }
        Some(std::fs::read_to_string(password_file)?)
    };
    let password = select_owner_password(environment.as_deref(), file_contents.as_deref())?;
    let hash = tokio::task::spawn_blocking(move || passwords::hash(&password))
        .await
        .map_err(|_| io::Error::other("Password bootstrap worker did not complete"))??;
    control
        .initialize(bootstrap.username.trim(), &hash, now_ms()?)
        .await?;
    Ok(control)
}

pub async fn from_environment(vaults: &[PathBuf]) -> io::Result<Option<Arc<Control>>> {
    let Some(profile) = std::env::var_os("LAPIS_SERVER_DATA_ROOT") else {
        return Ok(None);
    };
    let password_file =
        std::env::var_os("LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE").map(PathBuf::from);
    let environment = std::env::var("LAPIS_SERVER_BOOTSTRAP_PASSWORD").ok();
    let bootstrap = std::env::var("LAPIS_SERVER_BOOTSTRAP_USERNAME")
        .ok()
        .filter(|_| {
            password_source_message(environment.as_deref()) == "owner password source: environment"
                || password_file.is_some()
        })
        .map(|username| Bootstrap {
            username,
            password_file,
        });
    open(&PathBuf::from(profile), vaults, bootstrap)
        .await
        .map(Some)
}

#[cfg(test)]
#[path = "bootstrap_password.rs"]
mod bootstrap_password;
