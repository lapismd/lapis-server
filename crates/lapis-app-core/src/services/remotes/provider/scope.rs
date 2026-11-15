use super::*;
use jj_forge::accounts::{Account, api_root, forgejo_api_root, store::account_id};
use reqwest::Url;

pub(super) fn origin(host: &str) -> Result<String> {
    let value = format!("https://{host}");
    let url = Url::parse(&value).map_err(|_| invalid())?;
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.origin().ascii_serialization().to_lowercase() != value.to_lowercase()
    {
        return Err(invalid());
    }
    Ok(url.origin().ascii_serialization())
}
pub(super) fn connection(host: &str, value: &str) -> Result<()> {
    let url = Url::parse(value).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.origin().ascii_serialization() != origin(host)?
    {
        return Err(invalid());
    }
    Ok(())
}
pub(super) fn account_host(account: &Account) -> Result<String> {
    let api = api_root(&account.api_url)
        .or_else(|_| forgejo_api_root(&account.api_url))
        .map_err(forge_error)?;
    if api != account.api_url || account.id != account_id(&api, &account.user_id, &account.method) {
        return Err(invalid());
    }
    if api == "https://api.github.com" {
        return Ok("github.com".into());
    }
    let url = Url::parse(&api).map_err(|_| invalid())?;
    Ok(url
        .origin()
        .ascii_serialization()
        .trim_start_matches("https://")
        .into())
}
pub(super) fn api(input: &RemoteCredentialInput) -> Result<String> {
    let origin = origin(&input.host)?;
    match input.provider {
        RemoteCredentialProvider::Github if origin == "https://github.com" => {
            Ok("https://api.github.com".into())
        }
        RemoteCredentialProvider::Github => {
            api_root(&format!("{origin}/api/v3")).map_err(forge_error)
        }
        RemoteCredentialProvider::Forgejo => {
            forgejo_api_root(&format!("{origin}/api/v1")).map_err(forge_error)
        }
        _ => Err(invalid()),
    }
}
