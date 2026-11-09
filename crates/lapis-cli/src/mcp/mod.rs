mod errors;
mod relay;
mod stdio;

use crate::{args::Cli, client, credential, errors::failure};
use lapis_app_protocol::content::{ContentError, ContentErrorCode};
use rmcp::{
    ClientCacheConfig, ClientLifecycleMode, ClientServiceExt,
    model::ProtocolVersion,
    transport::{
        StreamableHttpClientTransport, async_rw::AsyncRwTransport,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

pub(crate) async fn run(args: &Cli) -> Result<(), ContentError> {
    if args.json || args.token_stdin || args.token_file.is_none() {
        return Err(failure(ContentErrorCode::InvalidRequest));
    }
    let mut target = client::target(args.target.as_deref())?;
    target.set_path("/mcp");
    let token = credential::read(args)?;
    let timeout = Duration::from_millis(args.timeout_ms);
    let http = reqwest_mcp::Client::builder()
        .no_proxy()
        .redirect(reqwest_mcp::redirect::Policy::none())
        .retry(reqwest_mcp::retry::never())
        .timeout(timeout)
        .build()
        .map_err(|_| failure(ContentErrorCode::Unavailable))?;
    let transport = StreamableHttpClientTransport::with_client(
        http,
        StreamableHttpClientTransportConfig::with_uri(target.to_string())
            .auth_header(token)
            .max_concurrent_requests(64),
    );
    let upstream = tokio::time::timeout(
        timeout,
        ().serve_with_lifecycle(
            transport,
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        ),
    )
    .await
    .map_err(|_| failure(ContentErrorCode::Unavailable))?
    .map_err(errors::startup)?;
    upstream
        .peer()
        .set_response_cache_config(ClientCacheConfig::disabled())
        .await;
    let handler = relay::Relay {
        peer: upstream.peer().clone(),
        jobs: Arc::new(Semaphore::new(64)),
        timeout,
    };
    let closed = tokio_util::sync::CancellationToken::new();
    let output = stdio::Output {
        inner: tokio::io::stdout(),
        closed: closed.clone(),
    };
    let stdio =
        AsyncRwTransport::new_server(tokio::io::stdin(), output).with_max_line_length(64 * 1024);
    let outcome = match rmcp::service::serve_server_with_ct(handler, stdio, closed).await {
        Ok(server) => server
            .waiting()
            .await
            .map(|_| ())
            .map_err(|_| failure(ContentErrorCode::Unavailable)),
        Err(_) => Err(failure(ContentErrorCode::ProtocolMismatch)),
    };
    let _ = tokio::time::timeout(timeout, upstream.cancel()).await;
    outcome
}
