mod auth;
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
pub use identity::Authentication;
mod errors;
mod files;
mod management;
mod participants;
mod plugins;
mod queues;
mod repository_backup;
mod repository_restore;
mod repository_sources;
mod routes;
mod tasks;
mod transfers;
mod turns;

use crate::host::Host;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{delete, get, post},
};
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
    let mut router = Router::new()
        .route(
            "/__lapis/plugins/{client}/{plugin}/{version}/{fingerprint}/{*path}",
            get(plugins::serve),
        )
        .route(
            "/api/v3/vaults",
            get(routes::vaults).post(management::create),
        )
        .route("/api/v3/vaults/{vault}/name", post(management::rename))
        .route("/api/v3/vaults/{vault}/archive", post(management::archive))
        .route("/api/v3/vaults/{vault}/restore", post(management::restore))
        .route("/api/v3/vaults/import", post(transfers::import))
        .route("/api/v3/vaults/{vault}/export", get(transfers::export))
        .route("/api/v3/auth/current", get(browser::current))
        .route("/api/v3/auth/logout", post(browser::logout))
        .route("/api/v3/vaults/{vault}/clients", post(routes::attach))
        .route("/api/v3/vaults/{vault}", delete(routes::close))
        .route("/api/v3/clients/{client}/commands", post(routes::execute))
        .route(
            "/api/v3/clients/{client}/database",
            post(database::call).layer(DefaultBodyLimit::max(database::JSON_LIMIT)),
        )
        .route(
            "/api/v3/clients/{client}/database/events",
            get(database_events::observe),
        )
        .route(
            "/api/v3/clients/{client}/indexing",
            post(indexing::call).layer(DefaultBodyLimit::max(16 * 1024)),
        )
        .route(
            "/api/v3/clients/{client}/history/{method}",
            post(history_reader::call).layer(DefaultBodyLimit::max(128 * 1024)),
        )
        .route(
            "/api/v3/clients/{client}/history-index",
            post(history_index::call).layer(DefaultBodyLimit::max(64 * 1024)),
        )
        .route(
            "/api/v3/clients/{client}/tasks",
            post(tasks::call).layer(DefaultBodyLimit::max(64 * 1024)),
        )
        .route(
            "/api/v3/clients/{client}/repository-restore",
            post(repository_restore::call).layer(DefaultBodyLimit::max(16 * 1024)),
        )
        .route(
            "/api/v3/clients/{client}/repository-backup",
            post(repository_backup::call).layer(DefaultBodyLimit::max(16 * 1024)),
        )
        .route(
            "/api/v3/clients/{client}/repository-sources",
            post(repository_sources::call).layer(DefaultBodyLimit::max(16 * 1024)),
        )
        .route(
            "/api/v3/clients/{client}/participants",
            post(participants::call).layer(DefaultBodyLimit::max(
                lapis_app_protocol::participants::PARTICIPANT_REQUEST_LIMIT,
            )),
        )
        .route(
            "/api/v3/clients/{client}/queues",
            post(queues::call).layer(DefaultBodyLimit::max(
                lapis_app_protocol::queues::QUEUE_REQUEST_LIMIT,
            )),
        )
        .route(
            "/api/v3/clients/{client}/turns",
            post(turns::call).layer(DefaultBodyLimit::max(turns::JSON_LIMIT)),
        )
        .route(
            "/api/v3/clients/{client}/heartbeat",
            post(routes::heartbeat),
        )
        .route("/api/v3/clients/{client}", delete(routes::detach))
        .route(
            "/api/v3/clients/{client}/plugin-bundles/download",
            get(plugins::download),
        )
        .route(
            "/api/v3/clients/{client}/files/{*path}",
            get(files::read).put(files::write),
        )
        .route(
            "/api/v3/clients/{client}/resources/{*path}",
            get(files::resource),
        )
        .layer(DefaultBodyLimit::max(JSON_LIMIT))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::authorize,
        ))
        .route("/api/v3/system/info", get(browser::info))
        .route("/api/v3/auth/login", post(browser::login));
    if selected_web {
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
