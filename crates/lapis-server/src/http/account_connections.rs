use crate::control::{
    ConnectedAccount, ConnectionState, OAuthProvider, OAuthTokenResponse,
    ProviderUserInfo,
};
use crate::http::{State, errors::Failure};
use axum::{
    extract::{Path, State as AxumState},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use lapis_app_protocol::ErrorCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request to initiate an account connection
#[derive(Debug, Deserialize)]
pub struct InitiateConnectionRequest {
    pub provider_id: String,
    pub redirect_uri: String,
}

/// Response from initiating an account connection
#[derive(Debug, Serialize)]
pub struct InitiateConnectionResponse {
    pub authorization_url: String,
    pub state: String,
}

/// Request to complete an account connection
#[derive(Debug, Deserialize)]
pub struct CompleteConnectionRequest {
    pub state: String,
    pub code: String,
}

/// Response from completing an account connection
#[derive(Debug, Serialize)]
pub struct CompleteConnectionResponse {
    pub account: ConnectedAccountInfo,
}

/// Account information for API responses
#[derive(Debug, Serialize)]
pub struct ConnectedAccountInfo {
    pub id: String,
    pub provider_id: String,
    pub external_id: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub state: String,
    pub scopes: Vec<String>,
    pub connected_at: i64,
    pub last_used_at: i64,
}

impl From<ConnectedAccount> for ConnectedAccountInfo {
    fn from(account: ConnectedAccount) -> Self {
        Self {
            id: account.id.to_string(),
            provider_id: account.provider_id,
            external_id: account.external_id,
            display_name: account.display_name,
            email: account.email,
            state: match account.state {
                ConnectionState::Pending => "pending".to_string(),
                ConnectionState::Connected => "connected".to_string(),
                ConnectionState::Expired => "expired".to_string(),
                ConnectionState::Revoked => "revoked".to_string(),
            },
            scopes: account.scopes,
            connected_at: account.connected_at,
            last_used_at: account.last_used_at,
        }
    }
}

/// List all connected accounts
pub async fn list_connections(
    AxumState(state): AxumState<State>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::Io,
            "Control database is not available",
        )
    })?;

    let accounts = control
        .list_connections()
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to list connections: {}", e)))?;

    let account_infos: Vec<ConnectedAccountInfo> =
        accounts.into_iter().map(ConnectedAccountInfo::from).collect();

    Ok(Json(account_infos))
}

/// Initiate a new account connection
pub async fn initiate_connection(
    AxumState(state): AxumState<State>,
    Json(request): Json<InitiateConnectionRequest>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::Io,
            "Control database is not available",
        )
    })?;

    // Get provider configuration
    let provider = get_provider_config(&request.provider_id)?;

    let now = crate::control::now_ms()
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to get current time: {}", e)))?;

    let initiation = control
        .initiate_connection(&provider, &request.redirect_uri, now)
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to initiate connection: {}", e)))?;

    let response = InitiateConnectionResponse {
        authorization_url: initiation.authorization_url,
        state: initiation.state,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Complete an account connection with the authorization code
pub async fn complete_connection(
    AxumState(state): AxumState<State>,
    Json(request): Json<CompleteConnectionRequest>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::Io,
            "Control database is not available",
        )
    })?;

    // In a real implementation, we would:
    // 1. Look up the pending connection to get the provider_id
    // 2. Exchange the code for tokens with the provider
    // 3. Fetch user info from the provider
    // 4. Complete the connection

    // For now, we'll simulate this with mock data
    let token_response = OAuthTokenResponse {
        access_token: "mock-access-token".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: Some(3600),
        refresh_token: Some("mock-refresh-token".to_string()),
        scope: Some("read write".to_string()),
    };

    let user_info = ProviderUserInfo {
        id: "mock-user-id".to_string(),
        email: Some("user@example.com".to_string()),
        name: Some("Mock User".to_string()),
        avatar_url: None,
    };

    let now = crate::control::now_ms()
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to get current time: {}", e)))?;

    let account = control
        .complete_connection(&request.state, &request.code, &token_response, Some(&user_info), now)
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to complete connection: {}", e)))?;

    let response = CompleteConnectionResponse {
        account: ConnectedAccountInfo::from(account),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Disconnect an account
pub async fn disconnect_account(
    AxumState(state): AxumState<State>,
    Path(account_id): Path<String>,
) -> Result<impl IntoResponse, Failure> {
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::Io,
            "Control database is not available",
        )
    })?;

    let account_uuid = Uuid::parse_str(&account_id).map_err(|_| {
        Failure::new(
            ErrorCode::InvalidRequest,
            "Invalid account ID format",
        )
    })?;

    let now = crate::control::now_ms()
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to get current time: {}", e)))?;

    control
        .disconnect_account(account_uuid, now)
        .await
        .map_err(|e| Failure::new(ErrorCode::Io, &format!("Failed to disconnect account: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Get provider configuration by ID
fn get_provider_config(provider_id: &str) -> Result<OAuthProvider, Failure> {
    match provider_id {
        "github" => Ok(OAuthProvider {
            id: "github".to_string(),
            name: "GitHub".to_string(),
            authorization_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            userinfo_url: Some("https://api.github.com/user".to_string()),
            client_id: std::env::var("GITHUB_CLIENT_ID").unwrap_or_else(|_| "mock-client-id".to_string()),
            scopes: vec!["user:email".to_string(), "repo".to_string()],
        }),
        "google" => Ok(OAuthProvider {
            id: "google".to_string(),
            name: "Google".to_string(),
            authorization_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            userinfo_url: Some("https://www.googleapis.com/oauth2/v2/userinfo".to_string()),
            client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_else(|_| "mock-client-id".to_string()),
            scopes: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
        }),
        _ => Err(Failure::new(
            ErrorCode::InvalidRequest,
            &format!("Unknown provider: {}", provider_id),
        )),
    }
}
