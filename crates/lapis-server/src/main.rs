#![forbid(unsafe_code)]

use lapis_server::{
    config::Config,
    control::bootstrap,
    host::Host,
    http,
    lifecycle::{Shutdown, server::serve},
};
use std::{net::SocketAddr, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let config_path = args
        .next()
        .ok_or("Usage: lapis-server <config.json> [listen-address]")?;
    let address: SocketAddr = args
        .next()
        .unwrap_or_else(|| "127.0.0.1:8787".into())
        .parse()?;
    if args.next().is_some() {
        return Err("Unexpected server argument".into());
    }
    let config: Config = serde_json::from_slice(&std::fs::read(&config_path)?)?;
    let token = std::env::var("LAPIS_SERVER_TOKEN").ok();
    let roots = config
        .vaults
        .iter()
        .map(|vault| vault.root.clone())
        .collect::<Vec<_>>();
    let control = bootstrap::from_environment(&roots).await?;
    if control.is_some() && std::env::var_os("LAPIS_SERVER_ORIGIN").is_none() {
        return Err("Set LAPIS_SERVER_ORIGIN for browser authentication".into());
    }
    let authentication =
        match &control {
            Some(control) => http::Authentication::browser(
                control.clone(),
                token.as_deref(),
                std::env::var("LAPIS_SERVER_INSECURE_COOKIE").as_deref() == Ok("1"),
            )?,
            None => http::Authentication::token(token.as_deref().ok_or(
                "Set LAPIS_SERVER_TOKEN or configure the browser authentication profile",
            )?)?,
        };
    let origin =
        std::env::var("LAPIS_SERVER_ORIGIN").unwrap_or_else(|_| format!("http://{address}"));
    let host = Arc::new(match std::env::var_os("LAPIS_SERVER_DATA_ROOT") {
        Some(profile) => {
            let agents = match std::env::var_os("LAPIS_SERVER_AGENTS_CONFIG") {
                Some(path) => serde_json::from_slice(&std::fs::read(path)?)?,
                None => Vec::new(),
            };
            if std::env::var("LAPIS_SERVER_NATIVE_HISTORY").as_deref() == Ok("1") {
                Host::with_indexed_history(config, std::path::Path::new(&profile), agents)?
            } else {
                Host::with_profile_and_agents(config, std::path::Path::new(&profile), agents)?
            }
        }
        None => {
            if std::env::var_os("LAPIS_SERVER_AGENTS_CONFIG").is_some()
                || std::env::var("LAPIS_SERVER_NATIVE_HISTORY").as_deref() == Ok("1")
            {
                return Err("Native services require LAPIS_SERVER_DATA_ROOT".into());
            }
            Host::new(config)?
        }
    });
    if let (Some(control), Some(profile)) = (&control, std::env::var_os("LAPIS_SERVER_DATA_ROOT")) {
        host.enable_management(control.clone(), std::path::Path::new(&profile))
            .await?;
    }
    let web = match std::env::var_os("LAPIS_SERVER_STATIC_ROOT") {
        Some(root) => {
            let mut private = roots;
            private.push(std::path::PathBuf::from(config_path));
            for key in [
                "LAPIS_SERVER_DATA_ROOT",
                "LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE",
                "LAPIS_SERVER_AGENTS_CONFIG",
            ] {
                if let Some(path) = std::env::var_os(key) {
                    private.push(path.into());
                }
            }
            Some(http::StaticWeb::open(
                std::path::Path::new(&root),
                &private,
            )?)
        }
        None => None,
    };
    let listener = tokio::net::TcpListener::bind(address).await?;
    let origin = if address.port() == 0 && std::env::var_os("LAPIS_SERVER_ORIGIN").is_none() {
        format!("http://{}", listener.local_addr()?)
    } else {
        origin
    };
    let router = match web {
        Some(web) => http::router_with_web(host.clone(), authentication, &origin, web)?,
        None => http::router(host.clone(), authentication, &origin)?,
    };
    let shutdown = Shutdown::new()?;
    println!("Lapis file service listening on {}", listener.local_addr()?);
    let result = serve(listener, router, host, shutdown).await;
    if let Some(control) = control {
        control.close().await?;
    }
    result?;
    Ok(())
}
