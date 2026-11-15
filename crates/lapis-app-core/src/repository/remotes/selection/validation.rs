use super::{RemoteAuthentication, RemoteConnection, Target, invalid};
use std::io;

pub(super) fn validate(target: Target, connection: &RemoteConnection) -> io::Result<()> {
    jj_vault::RemoteTarget {
        url: connection.url.clone(),
        branch: connection.branch.clone(),
    }
    .validate()
    .map_err(|_| invalid("Invalid remote endpoint or branch"))?;
    if connection.name.is_empty()
        || connection.name.len() > 128
        || connection.name == "git"
        || connection.name.starts_with('-')
        || connection.name.starts_with('.')
        || connection.name.ends_with('.')
        || connection.name.contains("..")
        || connection.name.ends_with(".lock")
        || !connection
            .name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        || connection.bookmark.len() > 256
        || connection.bookmark.chars().any(char::is_control)
        || (target == Target::Original && connection.bookmark.is_empty())
        || (target == Target::Backup && connection.bookmark != "lapis-backup")
    {
        return Err(invalid("Invalid repository connection selection"));
    }
    let ssh = connection.url.starts_with("ssh://") || !connection.url.contains("://");
    match connection.authentication {
        RemoteAuthentication::Anonymous if !ssh && connection.credential.is_empty() => {}
        RemoteAuthentication::SshAgent | RemoteAuthentication::SshDefault
            if ssh && connection.credential.is_empty() => {}
        RemoteAuthentication::Token
            if !ssh
                && uuid::Uuid::parse_str(&connection.credential)
                    .is_ok_and(|id| !id.is_nil() && id.to_string() == connection.credential) => {}
        RemoteAuthentication::Forge
            if !ssh
                && connection
                    .credential
                    .strip_prefix("forge:")
                    .is_some_and(|id| {
                        id.len() == 64
                            && id
                                .bytes()
                                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                    }) => {}
        _ => {
            return Err(invalid(
                "Authentication does not match the selected endpoint or credential reference",
            ));
        }
    }
    Ok(())
}
