use super::{Control, IssuedSession, passwords};
use std::io;

#[derive(Debug)]
pub enum LoginError {
    Denied,
    Throttled,
    Store(io::Error),
}
impl From<io::Error> for LoginError {
    fn from(error: io::Error) -> Self {
        Self::Store(error)
    }
}
impl Control {
    pub async fn login(
        &self,
        username: &str,
        password: &str,
        peer: &str,
        now: i64,
    ) -> Result<IssuedSession, LoginError> {
        if username.is_empty()
            || username.encode_utf16().count() > 200
            || password.is_empty()
            || password.encode_utf16().count() > 1024
        {
            return Err(LoginError::Denied);
        }
        let permit = self
            .login_slot
            .clone()
            .try_acquire_owned()
            .map_err(|_| LoginError::Throttled)?;
        let key = Self::login_key(username, peer);
        if !self.login_allowed(&key, now).await? {
            return Err(LoginError::Throttled);
        }
        let owner = self.owner().await?.ok_or(LoginError::Denied)?;
        let matches = username.eq_ignore_ascii_case(&owner.username);
        let password = password.to_owned();
        let (valid, _permit) = tokio::task::spawn_blocking(move || {
            // The native permit remains held even if the HTTP future disconnects.
            (
                passwords::verify(&password, &owner.password_hash) && matches,
                permit,
            )
        })
        .await
        .map_err(|_| io::Error::other("Password worker did not complete"))?;
        if !valid {
            self.record_failure(&key, now).await?;
            return Err(LoginError::Denied);
        }
        self.clear_failures(&key).await?;
        Ok(self.issue(now).await?)
    }
}
