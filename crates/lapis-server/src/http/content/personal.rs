use super::super::RegisterRoute as _;
use super::{Control, Host, State, controls::controlled, native};
use crate::{
    control::now_ms,
    http::{
        browser::store,
        errors::{Failure, blocking},
        routes::body,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State as AxumState, rejection::JsonRejection},
    http::StatusCode,
};
use lapis_app_core::services::content::PersistentGrant;
use lapis_app_protocol::{
    ErrorCode,
    content::{ContentErrorCode, CreatedContentGrant, PersonalTokenRequest, PersonalTokenSummary},
};
use lapis_app_protocol::{api_route, http_docs::json_body};
use lapis_content_http::{CredentialResolver, Rejection, ResolveFuture};
use std::{collections::BTreeSet, io, sync::Arc, time::Duration};
use uuid::Uuid;

pub(super) fn routes() -> Router<State> {
    Router::new()
        .api(api_route!("/api/v3/content-access/tokens", Owner, { get(list) => ("List personal access tokens and expiry metadata", 200, None, json_body::<Vec<lapis_app_protocol::content::PersonalTokenSummary>>()), post(create) => ("Issue a persistent personal access token", 201, json_body::<lapis_app_protocol::content::PersonalTokenRequest>(), json_body::<lapis_app_protocol::content::CreatedContentGrant>()) }))
        .api(api_route!("/api/v3/content-access/tokens/{id}", Owner, { delete(revoke) => ("Revoke a personal access token", 204, None, None) }))
}

fn control(state: &State) -> Result<&Control, Failure> {
    state.auth.control.as_deref().ok_or_else(|| {
        Failure::new(
            ErrorCode::Unsupported,
            "Personal tokens require a persistent server profile",
        )
    })
}

async fn list(
    AxumState(state): AxumState<State>,
) -> Result<Json<Vec<PersonalTokenSummary>>, Failure> {
    controlled(state, |state| async move {
        Ok(Json(
            control(&state)?
                .personal_tokens(now_ms().map_err(store)?)
                .await
                .map_err(store)?,
        ))
    })
    .await
}

async fn create(
    AxumState(state): AxumState<State>,
    input: Result<Json<PersonalTokenRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreatedContentGrant>), Failure> {
    let mut request = body(input)?;
    if request.label.len() > 128
        || request.label.trim().is_empty()
        || request.label.chars().any(char::is_control)
        || request.vault_ids.is_empty()
        || request.vault_ids.len() > 20
        || request.vault_ids.iter().any(Uuid::is_nil)
        || request.vault_ids.iter().collect::<BTreeSet<_>>().len() != request.vault_ids.len()
        || request.permissions.iter().collect::<BTreeSet<_>>().len() != request.permissions.len()
    {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Invalid personal token label or scope",
        ));
    }
    request.label = request.label.trim().into();
    controlled(state, move |state| async move {
        let control = control(&state)?;
        if !state.host.content.grants.is_enabled().map_err(native)? {
            return Err(Failure::new(
                ErrorCode::Closed,
                "Content access is disabled",
            ));
        }
        let operations = state
            .host
            .content
            .operations
            .snapshot(&request.permissions)
            .map_err(native)?;
        let host = state.host.clone();
        let vault_ids = request.vault_ids.clone();
        blocking(move || {
            let _admitted = host.content.admission()?;
            host.content_vaults(&vault_ids)?;
            Ok(())
        })
        .await?;
        let (grant_id, token) = control
            .create_personal_token(request, operations, now_ms().map_err(store)?)
            .await
            .map_err(|error| {
                if error.kind() == io::ErrorKind::WouldBlock {
                    Failure::new(ErrorCode::Busy, "Personal token limit reached")
                } else {
                    store(error)
                }
            })?;
        Ok((
            StatusCode::CREATED,
            Json(CreatedContentGrant { grant_id, token }),
        ))
    })
    .await
}

async fn revoke(
    AxumState(state): AxumState<State>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Failure> {
    controlled(state, move |state| async move {
        control(&state)?
            .revoke_personal(id, now_ms().map_err(store)?)
            .await
            .map_err(store)?;
        blocking(move || state.host.content.grants.revoke(id).map_err(native)).await?;
        Ok(StatusCode::NO_CONTENT)
    })
    .await
}

pub(super) struct Resolver {
    pub control: Arc<Control>,
    pub host: Arc<Host>,
    pub mcp_resource: Option<String>,
}

impl CredentialResolver for Resolver {
    fn resolve<'a>(&'a self, bearer: &'a str, path: &'a str) -> ResolveFuture<'a> {
        Box::pin(async move {
            let oauth = bearer.starts_with("lo1_");
            if !oauth && !bearer.starts_with("lp1_") {
                return Ok(());
            }
            if bearer.len() != 47
                || !bearer.as_bytes()[4..]
                    .iter()
                    .all(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_')
            {
                return Err(denied());
            }
            let _accepted = self.host.content.accept().map_err(|_| unavailable())?;
            let _guard = self.host.content.gate.lock().await;
            if !self.host.ready() || !self.host.content.grants.is_enabled().unwrap_or(false) {
                return Err(denied());
            }
            let now = now_ms().map_err(|_| unavailable())?;
            let approval = if oauth {
                let resource = self
                    .mcp_resource
                    .as_deref()
                    .filter(|_| path == "/mcp")
                    .ok_or_else(denied)?;
                self.control.mcp_approval(bearer, resource, now).await
            } else {
                self.control.personal_approval(bearer, now).await
            }
            .map_err(|_| unavailable())?
            .ok_or_else(denied)?;
            let id = approval.summary.grant_id;
            if approval
                .summary
                .last_used_at
                .is_none_or(|last| now - last >= 60_000)
            {
                if oauth {
                    self.control.touch_mcp(id, now).await
                } else {
                    self.control.touch_personal(id, now).await
                }
                .map_err(|_| unavailable())?;
            }
            if self.host.content.grants.check_credential(bearer).is_ok() {
                return Ok(());
            }
            let host = self.host.clone();
            let bearer = bearer.to_owned();
            tokio::task::spawn_blocking(move || {
                let _admitted = host.content.admission().map_err(|_| unavailable())?;
                host.content.grants.prune_expired()?;
                let vaults = host
                    .content_vaults(&approval.summary.vault_ids)
                    .map_err(|_| denied())?;
                let remaining = approval.summary.expires_at.map_or(300_000, |deadline| {
                    deadline
                        .saturating_sub(now_ms().unwrap_or(i64::MAX))
                        .min(300_000)
                });
                if remaining <= 0 {
                    return Err(denied());
                }
                host.content.grants.lease(
                    PersistentGrant {
                        id,
                        label: &approval.summary.label,
                        bearer: &bearer,
                        permissions: &approval.summary.permissions,
                        operations: approval.operations,
                    },
                    vaults,
                    Duration::from_millis(remaining as u64),
                )?;
                Ok(())
            })
            .await
            .map_err(|_| unavailable())?
        })
    }
}

fn denied() -> Rejection {
    Rejection::new(
        ContentErrorCode::Unauthenticated,
        "Invalid content credential",
    )
}

fn unavailable() -> Rejection {
    Rejection::new(
        ContentErrorCode::Unavailable,
        "Content authentication unavailable",
    )
}
