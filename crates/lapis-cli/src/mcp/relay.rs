use super::errors;
use lapis_app_protocol::content::ContentErrorCode;
use rmcp::{
    ErrorData, Peer, RoleClient, RoleServer, ServerHandler, ServiceError, model::*,
    service::RequestContext,
};
use std::{borrow::Cow, future::Future, sync::Arc, time::Duration};
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct Relay {
    pub peer: Peer<RoleClient>,
    pub jobs: Arc<Semaphore>,
    pub timeout: Duration,
}
impl Relay {
    async fn forward<T>(
        &self,
        context: RequestContext<RoleServer>,
        work: impl Future<Output = Result<T, ServiceError>>,
    ) -> Result<T, ErrorData> {
        let _permit = self
            .jobs
            .clone()
            .try_acquire_owned()
            .map_err(|_| errors::local(ContentErrorCode::Busy))?;
        tokio::select! {
            biased;
            _ = context.ct.cancelled() => Err(errors::local(ContentErrorCode::Unavailable)),
            result = tokio::time::timeout(self.timeout, work) => result
                .map_err(|_| errors::local(ContentErrorCode::Unavailable))?.map_err(errors::request),
        }
    }
}
impl ServerHandler for Relay {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new(
            "lapis-stdio",
            env!("CARGO_PKG_VERSION"),
        ))
    }
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&[ProtocolVersion::V_2026_07_28, ProtocolVersion::V_2025_11_25])
    }
    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let mut result = self
            .forward(context, self.peer.list_tools(page(request)))
            .await?;
        result.ttl_ms = Some(0);
        result.cache_scope = Some(CacheScope::Private);
        Ok(result)
    }
    async fn call_tool(
        &self,
        mut request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if request.input_responses.is_some() || request.request_state.is_some() {
            return Err(ErrorData::invalid_params(
                "Interactive tool continuations are not supported",
                None,
            ));
        }
        request.meta = None;
        // One upstream request: never drive MRTR follow-ups or replay a tool call.
        match self
            .forward(context, self.peer.call_tool_once(request))
            .await?
        {
            CallToolResponse::Complete(result) => Ok(result.into()),
            _ => Err(errors::local(ContentErrorCode::Unsupported)),
        }
    }
    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let mut result = self
            .forward(context, self.peer.list_resources(page(request)))
            .await?;
        result.ttl_ms = Some(0);
        result.cache_scope = Some(CacheScope::Private);
        Ok(result)
    }
    async fn list_resource_templates(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        let mut result = self
            .forward(context, self.peer.list_resource_templates(page(request)))
            .await?;
        result.ttl_ms = Some(0);
        result.cache_scope = Some(CacheScope::Private);
        Ok(result)
    }
    async fn read_resource(
        &self,
        mut request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        if request.input_responses.is_some() || request.request_state.is_some() {
            return Err(ErrorData::invalid_params(
                "Interactive resource continuations are not supported",
                None,
            ));
        }
        request.meta = None;
        match self
            .forward(context, self.peer.read_resource_once(request))
            .await?
        {
            ReadResourceResponse::Complete(mut result) => {
                result.ttl_ms = Some(0);
                result.cache_scope = Some(CacheScope::Private);
                Ok(result.into())
            }
            _ => Err(errors::local(ContentErrorCode::Unsupported)),
        }
    }
}
fn page(request: Option<PaginatedRequestParams>) -> Option<PaginatedRequestParams> {
    request.map(|mut page| {
        page.meta = None;
        page
    })
}
