use super::{HistoryMethod, policy::Policy};
use jj_history_service::{Context, Error, History, Result, frames, local, wire};
use std::sync::Arc;
use uuid::Uuid;

pub(super) async fn call(
    history: &Arc<History>,
    policy: &Policy,
    client: Uuid,
    method: HistoryMethod,
    frame: &str,
) -> Result<String> {
    let caller = Context {
        window: client.to_string(),
        url: super::policy::ORIGIN.into(),
    };
    match method {
        HistoryMethod::Heads => {
            let data: frames::Vault = wire::decode(frame)?;
            let vault = policy.vault(client, &data.vault)?;
            // Empty admission is read-only and does not create a repo or cache.
            if policy.resolve(client, &data.vault, &vault)?.is_none() {
                let (repository, generation) = Policy::identity(&vault, None);
                policy.vault(client, &data.vault)?;
                return wire::encode(local::Heads {
                    version: 1,
                    profile: local::PROFILE,
                    repository,
                    generation,
                    heads: vec![],
                });
            }
            wire::encode(history.heads(&caller, &data.vault).await?)
        }
        HistoryMethod::Open => {
            let data: frames::Open = wire::decode(frame)?;
            wire::encode(history.open(&caller, &data.vault, data.operation).await?)
        }
        HistoryMethod::Status | HistoryMethod::Close => {
            let data: frames::Selection = wire::decode(frame)?;
            policy.vault(client, &data.vault)?;
            if method == HistoryMethod::Close {
                wire::encode(history.close(&caller, &data.vault, data.handle).await?)
            } else {
                wire::encode(history.status(&caller, &data.vault, data.handle).await?)
            }
        }
        HistoryMethod::Read => {
            let data: frames::Read = wire::decode(frame)?;
            wire::encode(
                history
                    .read(&caller, &data.vault, data.handle, data.query)
                    .await?,
            )
        }
        HistoryMethod::Blame => {
            let data: frames::Blame = wire::decode(frame)?;
            wire::encode(
                history
                    .blame(&caller, &data.vault, data.handle, data.query)
                    .await?,
            )
        }
    }
}
impl std::str::FromStr for HistoryMethod {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "heads" => Ok(Self::Heads),
            "open" => Ok(Self::Open),
            "status" => Ok(Self::Status),
            "read" => Ok(Self::Read),
            "blame" => Ok(Self::Blame),
            "close" => Ok(Self::Close),
            _ => Err(Error::Invalid),
        }
    }
}
