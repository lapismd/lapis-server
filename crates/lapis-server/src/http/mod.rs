use lapis_app_protocol::{
    api_route,
    http_docs::{bytes, json_body, text},
};
lapis_app_protocol::api_router_extension!();
mod auth;
mod openapi;
pub use openapi::openapi_document;
mod cli;
mod content;
mod sync_account;
mod sync_account_recovery;
mod sync_discovery;
pub use content::restore_settings as restore_content_settings;
mod static_web;
pub use static_web::StaticWeb;
mod browser;
mod cookies;
mod database;
mod database_events;
mod history_index;
mod history_reader;
mod identity;
mod indexing;
mod mcp_oauth;
mod passkeys;
pub use identity::Authentication;
mod errors;
mod file_events;
mod files;
mod management;
mod participants;
mod plugins;
mod queues;
mod repository_backup;
mod repository_remotes;
mod repository_restore;
mod repository_sources;
mod routes;
mod tasks;
mod terminal;
mod transfers;
mod turns;

use crate::host::Host;
use axum::{Router, extract::DefaultBodyLimit, middleware};
use std::sync::Arc;

pub const JSON_LIMIT: usize = 2 * 1024 * 1024;
pub const VERSION_HEADER: &str = "x-lapis-protocol";

#[derive(Clone)]
struct State {
    host: Arc<Host>,
    auth: Authentication,
    build_id: String,
    origin: String,
    downloads: reqwest::Client,
    passkeys: Option<Arc<passkeys::Passkeys>>,
    oauth: Option<Arc<mcp_oauth::OAuth>>,
    web: Option<Arc<StaticWeb>>,
    observations: Arc<tokio::sync::Semaphore>,
}

pub fn router(host: Arc<Host>, auth: Authentication, origin: &str) -> std::io::Result<Router> {
    configured_router(host, auth, origin, None)
}

pub fn router_with_web(
    host: Arc<Host>,
    auth: Authentication,
    origin: &str,
    web: StaticWeb,
) -> std::io::Result<Router> {
    configured_router(host, auth, origin, Some(web))
}

fn configured_router(
    host: Arc<Host>,
    auth: Authentication,
    origin: &str,
    web: Option<StaticWeb>,
) -> std::io::Result<Router> {
    let uri: axum::http::Uri = origin
        .parse()
        .map_err(|_| std::io::Error::other("Invalid public origin"))?;
    if !matches!(uri.scheme_str(), Some("http" | "https"))
        || uri.authority().is_none()
        || uri
            .path_and_query()
            .is_some_and(|path| path.as_str() != "/")
        || origin.ends_with('/')
        || origin.contains('@')
    {
        return Err(std::io::Error::other(
            "Configure an exact HTTP(S) public origin",
        ));
    }
    auth.validate(&uri)?;
    let selected_web = web.is_some();
    let state = State {
        observations: Arc::new(tokio::sync::Semaphore::new(64)),
        web: web.map(Arc::new),
        oauth: (auth.control.is_some() && uri.scheme_str() == Some("https"))
            .then(|| Arc::new(mcp_oauth::OAuth::default())),
        passkeys: auth
            .control
            .as_ref()
            .and_then(|_| passkeys::Passkeys::new(origin))
            .map(Arc::new),
        downloads: plugins::client()?,
        host,
        auth,
        build_id: std::env::var("LAPIS_SERVER_BUILD_ID")
            .unwrap_or_else(|_| "development".into())
            .chars()
            .take(256)
            .collect(),
        origin: origin.into(),
    };
    let transport = content::transport(&state)?;
    let mut router = Router::new()
        .merge(content::routes())
        .merge(passkeys::owner_routes())
        .merge(mcp_oauth::owner_routes())
        .api(api_route!("/api/v3/cli/downloads", Owner, { get(cli::downloads) => ("List available CLI artifacts", 200, None, json_body::<serde_json::Value>()) }))
        .api(api_route!("/__lapis/plugins/{client}/{plugin}/{version}/{fingerprint}/{*path}", Owner, { get(plugins::serve) => ("Read an asset of an admitted plugin bundle", 200, None, bytes("application/octet-stream")) }))
        .api(api_route!("/api/v3/vaults", Owner, { get(routes::vaults) => ("List server vaults", 200, None, json_body::<Vec<lapis_app_protocol::ManagedVault>>()), post(management::create) => ("Create a managed vault", 201, json_body::<lapis_app_protocol::browser::VaultName>(), json_body::<lapis_app_protocol::ManagedVault>()) }))
        .api(api_route!("/api/v3/vaults/{vault}/name", Owner, { post(management::rename) => ("Rename a vault", 200, json_body::<lapis_app_protocol::browser::VaultName>(), json_body::<lapis_app_protocol::ManagedVault>()) }))
        .api(api_route!("/api/v3/vaults/{vault}/archive", Owner, { post(management::archive) => ("Archive a managed vault", 200, None, json_body::<lapis_app_protocol::ManagedVault>()) }))
        .api(api_route!("/api/v3/vaults/{vault}/restore", Owner, { post(management::restore) => ("Restore an archived vault", 200, None, json_body::<lapis_app_protocol::ManagedVault>()) }))
        .api(api_route!("/api/v3/vaults/import", Owner, { post(transfers::import) => ("Import a ZIP archive to a managed vault", 201, bytes("application/zip"), json_body::<lapis_app_protocol::ManagedVault>()) }))
        .api(api_route!("/api/v3/vaults/{vault}/export", Owner, { get(transfers::export) => ("Export a vault as a ZIP archive", 200, None, bytes("application/zip")) }))
        .api(api_route!("/api/v3/auth/current", Browser, { get(browser::current) => ("Read the current browser session", 200, None, json_body::<lapis_app_protocol::browser::SessionInfo>()) }))
        .api(api_route!("/api/v3/auth/logout", Browser, { post(browser::logout) => ("Sign out of the browser session", 200, None, json_body::<lapis_app_protocol::browser::LogoutReply>()) }))
        .api(api_route!("/api/v3/sync/account/{id}/decision", Browser, { post(sync_account::decide) => ("Approve or reject an account connection", 200, json_body::<serde_json::Value>(), json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/sync/account/{id}", Browser, { get(sync_account::inspect) => ("Inspect an account connection request", 200, None, json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/vaults/{vault}/clients", Owner, { post(routes::attach) => ("Attach an application client", 200, json_body::<lapis_app_protocol::AttachClient>(), json_body::<lapis_app_protocol::ClientDescriptor>()) }))
        .api(api_route!("/api/v3/vaults/{vault}", Owner, { delete(routes::close) => ("Close the native vault runtime", 204, None, None) }))
        .api(api_route!("/api/v3/clients/{client}/commands", Owner, { post(routes::execute) => ("Run a vault attachment command", 200, json_body::<lapis_app_protocol::Request>(), json_body::<lapis_app_protocol::Reply>()) }))
        .api(api_route!("/api/v3/clients/{client}/repository-remotes", Owner, { get(repository_remotes::settings) => ("Repository synchronization settings", 200, None, json_body::<lapis_app_protocol::repository_sync::RepositorySyncSnapshot>()), post(repository_remotes::control) => ("Update repository synchronization", 200, json_body::<lapis_app_protocol::repository_sync::RepositorySyncCommand>(), json_body::<lapis_app_protocol::repository_sync::RepositorySyncSnapshot>()) }, layer = DefaultBodyLimit::max(64 * 1024)))
        .api(api_route!("/api/v3/clients/{client}/repository-working-copy", Owner, { get(repository_remotes::working_copy) => ("Read working copy status", 200, None, json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/clients/{client}/repository-synchronization", Owner, { get(repository_remotes::synchronization) => ("Read repository remotes", 200, None, json_body::<lapis_app_protocol::repository_remotes::RepositoryRemotesSnapshot>()), post(repository_remotes::control_synchronization) => ("Update repository remotes", 200, json_body::<lapis_app_protocol::repository_remotes::RepositoryRemotesCommand>(), json_body::<lapis_app_protocol::repository_remotes::RepositoryRemotesSnapshot>()) }, layer = DefaultBodyLimit::max(1024 * 1024 + 16 * 1024)))
        .api(api_route!("/api/v3/clients/{client}/database", Owner, { post(database::call) => ("Run a database command", 200, json_body::<lapis_app_protocol::database::call::DatabaseCall>(), json_body::<lapis_app_protocol::database::call::DatabaseValue>()) }, layer = DefaultBodyLimit::max(database::JSON_LIMIT)))
        .api(api_route!("/api/v3/clients/{client}/file-events", Owner, { get(file_events::observe) => ("Observe file changes", 200, None, text("text/event-stream")) }))
        .api(api_route!("/api/v3/clients/{client}/database/events", Owner, { get(database_events::observe) => ("Observe database events", 200, None, text("text/event-stream")) }))
        .api(api_route!("/api/v3/clients/{client}/indexing", Owner, { post(indexing::call) => ("Run an indexing command", 200, json_body::<lapis_app_protocol::indexing::IndexingCommand>(), json_body::<lapis_app_protocol::indexing::IndexingValue>()) }, layer = DefaultBodyLimit::max(16 * 1024)))
        .api(api_route!("/api/v3/clients/{client}/history/{method}", Owner, { post(history_reader::call) => ("Run a history query", 200, json_body::<serde_json::Value>(), json_body::<serde_json::Value>()) }, layer = DefaultBodyLimit::max(128 * 1024)))
        .api(api_route!("/api/v3/clients/{client}/history-index", Owner, { post(history_index::call) => ("Run a history index command", 200, json_body::<lapis_app_protocol::history_index::HistoryIndexCommand>(), json_body::<lapis_app_protocol::history_index::HistoryIndexValue>()) }, layer = DefaultBodyLimit::max(64 * 1024)))
        .api(api_route!("/api/v3/clients/{client}/tasks", Owner, { post(tasks::call) => ("Run a Tasks command", 200, json_body::<lapis_app_protocol::native_tasks::NativeTaskCommand>(), json_body::<lapis_app_protocol::native_tasks::NativeTaskValue>()) }, layer = DefaultBodyLimit::max(64 * 1024)))
        .api(api_route!("/api/v3/clients/{client}/repository-restore", Owner, { post(repository_restore::call) => ("Run a repository recovery command", 200, json_body::<lapis_app_protocol::repository_restore::RepositoryRestoreCommand>(), json_body::<lapis_app_protocol::repository_restore::RepositoryRestoreValue>()) }, layer = DefaultBodyLimit::max(16 * 1024)))
        .api(api_route!("/api/v3/clients/{client}/repository-backup", Owner, { post(repository_backup::call) => ("Run a repository backup command", 200, json_body::<lapis_app_protocol::repository_backup::RepositoryBackupCommand>(), json_body::<lapis_app_protocol::repository_backup::RepositoryBackupSnapshot>()) }, layer = DefaultBodyLimit::max(16 * 1024)))
        .api(api_route!("/api/v3/clients/{client}/repository-sources", Owner, { post(repository_sources::call) => ("Run a repository source command", 200, json_body::<lapis_app_protocol::repository_sources::RepositorySourcesCommand>(), json_body::<serde_json::Value>()) }, layer = DefaultBodyLimit::max(16 * 1024)))
        .api(api_route!("/api/v3/clients/{client}/participants", Owner, { post(participants::call) => ("Run a participant command", 200, json_body::<lapis_app_protocol::participants::ParticipantCommand>(), json_body::<lapis_app_protocol::participants::ParticipantValue>()) }, layer = DefaultBodyLimit::max(
                lapis_app_protocol::participants::PARTICIPANT_REQUEST_LIMIT,
            )))
        .api(api_route!("/api/v3/clients/{client}/queues", Owner, { post(queues::call) => ("Run a queue command", 200, json_body::<lapis_app_protocol::queues::QueueCommand>(), json_body::<lapis_app_protocol::queues::QueueValue>()) }, layer = DefaultBodyLimit::max(
                lapis_app_protocol::queues::QUEUE_REQUEST_LIMIT,
            )))
        .api(api_route!("/api/v3/clients/{client}/turns", Owner, { post(turns::call) => ("Run an agent turn command", 200, json_body::<lapis_app_protocol::turns::TurnCommand>(), json_body::<lapis_app_protocol::turns::TurnValue>()) }, layer = DefaultBodyLimit::max(turns::JSON_LIMIT)))
        .api(api_route!("/api/v3/clients/{client}/heartbeat", Owner, { post(routes::heartbeat) => ("Renew an application attachment", 204, None, None) }))
        .api(api_route!("/api/v3/clients/{client}/terminal", Owner, { post(terminal::call) => ("Run a terminal command", 200, json_body::<lapis_app_protocol::terminal::TerminalCommand>(), json_body::<lapis_app_protocol::terminal::TerminalValue>()) }, layer = DefaultBodyLimit::max(512 * 1024)))
        .api(api_route!("/api/v3/clients/{client}/terminal/{session}/events", Owner, { get(terminal::observe) => ("Observe terminal output", 200, None, text("text/event-stream")) }))
        .api(api_route!("/api/v3/clients/{client}", Owner, { delete(routes::detach) => ("Detach an application client", 204, None, None) }))
        .api(api_route!("/api/v3/clients/{client}/operations", Owner, { get(plugin_operations::catalog) => ("List enabled plugin operations", 200, None, json_body::<lapis_app_protocol::operations::OperationCatalog>()) }))
        .api(api_route!("/api/v3/clients/{client}/plugins/{plugin}/operations/{operation}", Owner, { post(plugin_operations::invoke) => ("Invoke a plugin operation", 200, json_body::<lapis_app_protocol::operations::OperationRequest>(), json_body::<lapis_app_protocol::operations::OperationReply>()) }, layer = DefaultBodyLimit::max(64 * 1024)))
        .api(api_route!("/api/v3/clients/{client}/plugin-bundles/download", Owner, { get(plugins::download) => ("Download and verify a plugin bundle", 200, json_body::<serde_json::Value>(), json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/clients/{client}/files/{*path}", Owner, { get(files::read) => ("Read a file with its current ETag", 200, None, bytes("application/octet-stream")), put(files::write) => ("Replace a file with a conditional ETag", 200, bytes("application/octet-stream"), None) }))
        .api(api_route!("/api/v3/clients/{client}/resources/{*path}", Owner, { get(files::resource) => ("Read an admitted workspace resource", 200, None, bytes("application/octet-stream")) }))
        .layer(DefaultBodyLimit::max(JSON_LIMIT))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::authorize,
        ))
        .api(api_route!("/api/v3/system/info", Public, { get(browser::info) => ("Read the server protocol and build identity", 200, None, json_body::<lapis_app_protocol::browser::HostInfo>()) }))
        .api(api_route!("/api/v3/auth/login", Login, { post(browser::login) => ("Sign in with a password", 200, json_body::<lapis_app_protocol::browser::LoginRequest>(), json_body::<lapis_app_protocol::browser::SessionInfo>()) }))
        .merge(passkeys::public_routes())
        .merge(mcp_oauth::public_routes())
        .api(api_route!("/icon.png", Public, { get(mcp_oauth::icon) => ("Fetch the public MCP server logo", 200, None, bytes("image/png")) }))
        .api(api_route!("/api/v3/sync/account", Sync, { post(sync_account::initiate) => ("Start native account approval", 200, json_body::<serde_json::Value>(), json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/sync/account/{id}/redeem", Sync, { post(sync_account::redeem) => ("Exchange an approved native account request", 200, json_body::<serde_json::Value>(), json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/sync/account/{id}/inspect", Sync, { post(sync_account_recovery::inspect) => ("Inspect the outcome of native account approval", 200, json_body::<serde_json::Value>(), json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/sync/account/{id}/cancel", Sync, { post(sync_account_recovery::cancel) => ("Cancel a native account connection", 200, json_body::<serde_json::Value>(), json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/sync/discovery/vaults", Sync, { get(sync_discovery::vaults) => ("List account vault discovery advertisements", 200, None, json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/sync/discovery/sign-out", Sync, { post(sync_discovery::sign_out) => ("Revoke this account discovery credential", 200, None, json_body::<serde_json::Value>()) }))
        .merge(transport)
        .merge(openapi::routes());
    if selected_web {
        lapis_app_protocol::api_operation!(
            "/healthz",
            Public,
            "get",
            "Check server liveness",
            200,
            None,
            text("text/plain")
        );
        lapis_app_protocol::api_operation!(
            "/readyz",
            Public,
            "get",
            "Check server readiness",
            200,
            None,
            text("text/plain")
        );
        lapis_app_protocol::api_operation!(
            "/",
            Public,
            "get",
            "Open the hosted application",
            200,
            None,
            text("text/html")
        );
        lapis_app_protocol::api_operation!(
            "/{path}",
            Public,
            "get",
            "Serve hosted static files or the browser application",
            200,
            None,
            bytes("application/octet-stream")
        );
        router = router
            .route("/healthz", axum::routing::any(static_web::health))
            .route("/readyz", axum::routing::any(static_web::ready))
            .fallback(static_web::serve)
            .layer(middleware::from_fn_with_state(
                state.clone(),
                static_web::available,
            ));
    }
    Ok(router
        .layer(middleware::from_fn(auth::response_headers))
        .with_state(state))
}
mod plugin_operations;
