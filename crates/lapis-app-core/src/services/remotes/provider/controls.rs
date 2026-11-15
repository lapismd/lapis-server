use super::*;
use jj_forge::accounts::Command;
use zeroize::Zeroizing;
impl NativeRemoteCredentials {
    pub(super) async fn control_native(&self, action: RemoteCredentialAction) -> Result<()> {
        if matches!(action, RemoteCredentialAction::CancelSignIn) {
            let current = self
                .authorization
                .lock()
                .map_err(|_| unavailable())?
                .clone();
            if let Some(current) = current {
                self.forges
                    .execute(
                        Command::CancelOauth {
                            session: current.session,
                        },
                        jj_forge::accounts::now().map_err(forge_error)?,
                    )
                    .await
                    .map_err(forge_error)?;
            }
            *self.authorization.lock().map_err(|_| unavailable())? = None;
            return Ok(());
        }
        let settings = self.inventory()?;
        if !matches!(settings.storage, RemoteStorageState::Available) {
            return Err(unavailable());
        }
        super::super::custody::preflight(&settings, &action)?;
        let now = jj_forge::accounts::now().map_err(forge_error)?;
        match action {
            RemoteCredentialAction::Save(mut input) => {
                // The transient Protocol input is never persisted or printable.
                let token = Zeroizing::new(std::mem::take(&mut input.token));
                if matches!(input.provider, RemoteCredentialProvider::Https) {
                    input.token = token.to_string();
                    let result = self.tokens.save(&input).map_err(|_| unavailable());
                    let _clear = Zeroizing::new(std::mem::take(&mut input.token));
                    return result;
                }
                let api = scope::api(&input)?;
                if let Some(id) = &input.replace {
                    let account = self.forges.account(id).map_err(forge_error)?;
                    if scope::account_host(&account)?.to_lowercase() != input.host.to_lowercase()
                        || account.api_url != api
                        || account.login != input.username
                    {
                        return Err(invalid());
                    }
                    self.forges
                        .replace_token(id, token.to_string())
                        .await
                        .map_err(forge_error)?;
                } else if matches!(input.provider, RemoteCredentialProvider::Forgejo) {
                    self.forges
                        .add_forgejo_token(&api, token.to_string())
                        .await
                        .map_err(forge_error)?;
                } else {
                    self.forges
                        .execute(
                            Command::AddToken {
                                api_url: api,
                                token: token.to_string(),
                            },
                            now,
                        )
                        .await
                        .map_err(forge_error)?;
                }
            }
            RemoteCredentialAction::Forget(id) => {
                let selected = settings
                    .credentials
                    .iter()
                    .find(|c| c.id == id)
                    .ok_or_else(invalid)?;
                if matches!(selected.method, RemoteCredentialMethod::Token) {
                    self.tokens.forget(&id).map_err(|_| unavailable())?;
                } else {
                    self.forges
                        .execute(Command::Forget { account: id }, now)
                        .await
                        .map_err(forge_error)?;
                }
            }
            RemoteCredentialAction::BeginSignIn => {
                if self
                    .authorization
                    .lock()
                    .map_err(|_| unavailable())?
                    .as_ref()
                    .is_some_and(|a| a.state == "pending" && a.expires_at > now)
                {
                    return Err(Error::new(
                        ErrorCode::Busy,
                        "Complete or cancel the current sign-in",
                    ));
                }
                let started = self
                    .forges
                    .execute(Command::BeginOauth, now)
                    .await
                    .map_err(forge_error)?;
                *self.authorization.lock().map_err(|_| unavailable())? = started.authorization;
            }
            RemoteCredentialAction::CheckSignIn => {
                let selected = self
                    .authorization
                    .lock()
                    .map_err(|_| unavailable())?
                    .clone()
                    .ok_or_else(invalid)?;
                let checked = self
                    .forges
                    .execute(
                        Command::PollOauth {
                            session: selected.session,
                        },
                        now,
                    )
                    .await
                    .map_err(forge_error)?;
                *self.authorization.lock().map_err(|_| unavailable())? = checked.authorization;
            }
            RemoteCredentialAction::CancelSignIn => unreachable!(),
        }
        Ok(())
    }
}
