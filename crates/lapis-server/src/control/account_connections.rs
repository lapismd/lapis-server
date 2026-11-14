use super::{Control, engine, passwords, unavailable, validate_time};
use serde::{Deserialize, Serialize};
use std::io;
use uuid::Uuid;

/// OAuth 2.0 provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProvider {
    pub id: String,
    pub name: String,
    pub authorization_url: String,
    pub token_url: String,
    pub userinfo_url: Option<String>,
    pub client_id: String,
    pub scopes: Vec<String>,
}

/// Account connection state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionState {
    Pending,
    Connected,
    Expired,
    Revoked,
}

/// Connected external account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedAccount {
    pub id: Uuid,
    pub provider_id: String,
    pub external_id: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub state: ConnectionState,
    pub access_token_hash: String,
    pub refresh_token_hash: Option<String>,
    pub token_expires_at: Option<i64>,
    pub scopes: Vec<String>,
    pub connected_at: i64,
    pub last_used_at: i64,
}

/// OAuth authorization request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationRequest {
    pub provider_id: String,
    pub state: String,
    pub code_challenge: String,
    pub redirect_uri: String,
}

/// OAuth token exchange request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenExchangeRequest {
    pub provider_id: String,
    pub code: String,
    pub code_verifier: String,
    pub redirect_uri: String,
}

/// OAuth token response from provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

/// User info from provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUserInfo {
    pub id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

/// Connection initiation result
pub struct ConnectionInitiation {
    pub authorization_url: String,
    pub state: String,
    pub code_verifier: String,
}

impl Control {
    /// Initiate account connection flow
    pub async fn initiate_connection(
        &self,
        provider: &OAuthProvider,
        redirect_uri: &str,
        now: i64,
    ) -> io::Result<ConnectionInitiation> {
        validate_time(now)?;

        // Generate PKCE code verifier and challenge
        let code_verifier = passwords::token()?;
        let code_challenge = passwords::sha256_base64url(&code_verifier);

        // Generate state parameter
        let state = passwords::token()?;

        // Build authorization URL
        let mut params = vec![
            ("response_type", "code"),
            ("client_id", &provider.client_id),
            ("redirect_uri", redirect_uri),
            ("state", &state),
            ("code_challenge", &code_challenge),
            ("code_challenge_method", "S256"),
        ];

        let scope_string = provider.scopes.join(" ");
        if !provider.scopes.is_empty() {
            params.push(("scope", &scope_string));
        }

        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let authorization_url = format!("{}?{}", provider.authorization_url, query);

        // Store pending connection
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        connection
            .execute(
                "INSERT INTO pending_connections(state, provider_id, code_verifier, redirect_uri, created_at) VALUES(?,?,?,?,?)",
                turso::params![
                    state.clone(),
                    provider.id.clone(),
                    code_verifier.clone(),
                    redirect_uri.to_string(),
                    now,
                ],
            )
            .await
            .map_err(engine)?;

        Ok(ConnectionInitiation {
            authorization_url,
            state,
            code_verifier,
        })
    }

    /// Complete account connection with authorization code
    pub async fn complete_connection(
        &self,
        state: &str,
        _code: &str,
        token_response: &OAuthTokenResponse,
        user_info: Option<&ProviderUserInfo>,
        now: i64,
    ) -> io::Result<ConnectedAccount> {
        validate_time(now)?;

        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        // Retrieve and validate pending connection
        let pending = {
            let mut rows = connection
                .query(
                    "SELECT provider_id, code_verifier, redirect_uri FROM pending_connections WHERE state=?",
                    turso::params![state],
                )
                .await
                .map_err(engine)?;

            rows.next()
                .await
                .map_err(engine)?
                .ok_or_else(|| io::Error::other("Invalid or expired connection state"))?
        };

        let provider_id: String = pending.get(0).map_err(engine)?;
        let _code_verifier: String = pending.get(1).map_err(engine)?;
        let _redirect_uri: String = pending.get(2).map_err(engine)?;

        // Delete pending connection
        connection
            .execute(
                "DELETE FROM pending_connections WHERE state=?",
                turso::params![state],
            )
            .await
            .map_err(engine)?;

        // Calculate token expiration
        let token_expires_at = token_response
            .expires_in
            .map(|expires_in| now + (expires_in * 1000));

        // Hash tokens for storage
        let access_token_hash = passwords::digest(&token_response.access_token);
        let refresh_token_hash = token_response
            .refresh_token
            .as_ref()
            .map(|t| passwords::digest(t));

        // Parse scopes
        let scopes: Vec<String> = token_response
            .scope
            .as_ref()
            .map(|s| s.split_whitespace().map(String::from).collect())
            .unwrap_or_default();

        // Generate account ID
        let account_id = Uuid::new_v4();

        // Extract user info
        let (external_id, display_name, email) = if let Some(info) = user_info {
            (
                info.id.clone(),
                info.name.clone(),
                info.email.clone(),
            )
        } else {
            (
                format!("unknown-{}", account_id),
                None,
                None,
            )
        };

        // Store connected account
        connection
            .execute(
                "INSERT INTO connected_accounts(id, owner_id, provider_id, external_id, display_name, email, state, access_token_hash, refresh_token_hash, token_expires_at, scopes, connected_at, last_used_at) VALUES(?,1,?,?,?,?,?,?,?,?,?,?,?)",
                turso::params![
                    account_id.to_string(),
                    provider_id.clone(),
                    external_id.clone(),
                    display_name.as_deref().map(|s| s.to_string()),
                    email.as_deref().map(|s| s.to_string()),
                    "connected".to_string(),
                    access_token_hash.clone(),
                    refresh_token_hash.clone(),
                    token_expires_at,
                    serde_json::to_string(&scopes).map_err(|e| io::Error::other(e.to_string()))?,
                    now,
                    now,
                ],
            )
            .await
            .map_err(engine)?;

        Ok(ConnectedAccount {
            id: account_id,
            provider_id,
            external_id,
            display_name,
            email,
            state: ConnectionState::Connected,
            access_token_hash,
            refresh_token_hash,
            token_expires_at,
            scopes,
            connected_at: now,
            last_used_at: now,
        })
    }

    /// List all connected accounts
    pub async fn list_connections(&self) -> io::Result<Vec<ConnectedAccount>> {
        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        let mut rows = connection
            .query(
                "SELECT id, provider_id, external_id, display_name, email, state, access_token_hash, refresh_token_hash, token_expires_at, scopes, connected_at, last_used_at FROM connected_accounts WHERE owner_id=1 ORDER BY connected_at DESC",
                (),
            )
            .await
            .map_err(engine)?;

        let mut accounts = Vec::new();
        while let Some(row) = rows.next().await.map_err(engine)? {
            let state_str: String = row.get(5).map_err(engine)?;
            let state = match state_str.as_str() {
                "pending" => ConnectionState::Pending,
                "connected" => ConnectionState::Connected,
                "expired" => ConnectionState::Expired,
                "revoked" => ConnectionState::Revoked,
                _ => ConnectionState::Revoked,
            };

            let scopes_json: String = row.get(9).map_err(engine)?;
            let scopes: Vec<String> = serde_json::from_str(&scopes_json)
                .map_err(|e| io::Error::other(e.to_string()))?;

            accounts.push(ConnectedAccount {
                id: Uuid::parse_str(&row.get::<String>(0).map_err(engine)?)
                    .map_err(|_| unavailable())?,
                provider_id: row.get(1).map_err(engine)?,
                external_id: row.get(2).map_err(engine)?,
                display_name: row.get(3).map_err(engine)?,
                email: row.get(4).map_err(engine)?,
                state,
                access_token_hash: row.get(6).map_err(engine)?,
                refresh_token_hash: row.get(7).map_err(engine)?,
                token_expires_at: row.get(8).map_err(engine)?,
                scopes,
                connected_at: row.get(10).map_err(engine)?,
                last_used_at: row.get(11).map_err(engine)?,
            });
        }

        Ok(accounts)
    }

    /// Disconnect an account
    pub async fn disconnect_account(&self, account_id: Uuid, now: i64) -> io::Result<()> {
        validate_time(now)?;

        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        let result = connection
            .execute(
                "UPDATE connected_accounts SET state='revoked' WHERE id=? AND owner_id=1 AND state='connected'",
                turso::params![account_id.to_string()],
            )
            .await
            .map_err(engine)?;

        if result == 0 {
            return Err(io::Error::other("Account not found or already disconnected"));
        }

        Ok(())
    }

    /// Update account last used timestamp
    pub async fn touch_account(&self, account_id: Uuid, now: i64) -> io::Result<()> {
        validate_time(now)?;

        let guard = self.connection.lock().await;
        let connection = guard.as_ref().ok_or_else(unavailable)?;

        connection
            .execute(
                "UPDATE connected_accounts SET last_used_at=? WHERE id=? AND owner_id=1",
                turso::params![now, account_id.to_string()],
            )
            .await
            .map_err(engine)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_control() -> (Control, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let control = Control::open(temp_dir.path()).await.unwrap();
        
        // Initialize the owner (required for foreign key constraints)
        let now = 1700000000000;
        let password_hash = passwords::hash("test-password-12-chars").unwrap();
        control.initialize("testuser", &password_hash, now).await.unwrap();
        
        (control, temp_dir)
    }

    #[tokio::test]
    async fn test_initiate_connection() {
        let (control, _temp_dir) = setup_control().await;

        let provider = OAuthProvider {
            id: "github".to_string(),
            name: "GitHub".to_string(),
            authorization_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            userinfo_url: Some("https://api.github.com/user".to_string()),
            client_id: "test-client-id".to_string(),
            scopes: vec!["user:email".to_string()],
        };

        let now = 1700000000000;
        let result = control
            .initiate_connection(&provider, "https://localhost/callback", now)
            .await
            .unwrap();

        assert!(result.authorization_url.contains("github.com"));
        assert!(result.authorization_url.contains("client_id=test-client-id"));
        assert!(!result.state.is_empty());
        assert!(!result.code_verifier.is_empty());
    }

    #[tokio::test]
    async fn test_complete_connection() {
        let (control, _temp_dir) = setup_control().await;

        let provider = OAuthProvider {
            id: "github".to_string(),
            name: "GitHub".to_string(),
            authorization_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            userinfo_url: Some("https://api.github.com/user".to_string()),
            client_id: "test-client-id".to_string(),
            scopes: vec!["user:email".to_string()],
        };

        let now = 1700000000000;
        let initiation = control
            .initiate_connection(&provider, "https://localhost/callback", now)
            .await
            .unwrap();

        let token_response = OAuthTokenResponse {
            access_token: "test-access-token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("test-refresh-token".to_string()),
            scope: Some("user:email".to_string()),
        };

        let user_info = ProviderUserInfo {
            id: "12345".to_string(),
            email: Some("test@example.com".to_string()),
            name: Some("Test User".to_string()),
            avatar_url: None,
        };

        let account = control
            .complete_connection(
                &initiation.state,
                "test-code",
                &token_response,
                Some(&user_info),
                now + 1000,
            )
            .await
            .unwrap();

        assert_eq!(account.provider_id, "github");
        assert_eq!(account.external_id, "12345");
        assert_eq!(account.email, Some("test@example.com".to_string()));
        assert_eq!(account.display_name, Some("Test User".to_string()));
        assert_eq!(account.state, ConnectionState::Connected);
    }

    #[tokio::test]
    async fn test_list_connections() {
        let (control, _temp_dir) = setup_control().await;

        let provider = OAuthProvider {
            id: "github".to_string(),
            name: "GitHub".to_string(),
            authorization_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            userinfo_url: None,
            client_id: "test-client-id".to_string(),
            scopes: vec![],
        };

        let now = 1700000000000;
        let initiation = control
            .initiate_connection(&provider, "https://localhost/callback", now)
            .await
            .unwrap();

        let token_response = OAuthTokenResponse {
            access_token: "test-token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: None,
            refresh_token: None,
            scope: None,
        };

        control
            .complete_connection(
                &initiation.state,
                "test-code",
                &token_response,
                None,
                now + 1000,
            )
            .await
            .unwrap();

        let accounts = control.list_connections().await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].provider_id, "github");
    }

    #[tokio::test]
    async fn test_disconnect_account() {
        let (control, _temp_dir) = setup_control().await;

        let provider = OAuthProvider {
            id: "github".to_string(),
            name: "GitHub".to_string(),
            authorization_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            userinfo_url: None,
            client_id: "test-client-id".to_string(),
            scopes: vec![],
        };

        let now = 1700000000000;
        let initiation = control
            .initiate_connection(&provider, "https://localhost/callback", now)
            .await
            .unwrap();

        let token_response = OAuthTokenResponse {
            access_token: "test-token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: None,
            refresh_token: None,
            scope: None,
        };

        let account = control
            .complete_connection(
                &initiation.state,
                "test-code",
                &token_response,
                None,
                now + 1000,
            )
            .await
            .unwrap();

        control
            .disconnect_account(account.id, now + 2000)
            .await
            .unwrap();

        let accounts = control.list_connections().await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].state, ConnectionState::Revoked);
    }
}
