use crate::{catalog::Catalog, resources};
use axum::http::request::Parts;
use lapis_app_protocol::content::*;
use lapis_content_http::Boundary;
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use serde_json::json;
use std::{borrow::Cow, sync::Arc};
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct Handler {
    pub boundary: Boundary,
    pub catalog: Arc<Catalog>,
}
impl Handler {
    async fn authorize(&self, context: &RequestContext<RoleServer>) -> Result<(), ErrorData> {
        self.boundary
            .check(parts(context)?)
            .await
            .map_err(|e| content_error(e.into_error()))?;
        Ok(())
    }
    async fn execute(
        &self,
        context: &RequestContext<RoleServer>,
        command: ContentCommand,
    ) -> Result<ContentReply, ErrorData> {
        let request = ContentRequest {
            api_version: CONTENT_API_MAJOR,
            request_id: Uuid::new_v4(),
            command,
        };
        self.boundary
            .execute(parts(context)?, request)
            .await
            .map_err(|e| content_error(e.into_error()))
    }
}
impl ServerHandler for Handler {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new(
            "lapis-content",
            env!("CARGO_PKG_VERSION"),
        ))
    }
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&[ProtocolVersion::V_2026_07_28])
    }
    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        self.authorize(&context).await?;
        no_cursor(request)?;
        Ok(ListToolsResult::with_all_items(self.catalog.tools.clone())
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.authorize(&context).await?;
        let command = self.catalog.command(&request.name, request.arguments)?;
        let reply = self.execute(&context, command).await?;
        let error = matches!(reply.outcome, ContentOutcome::Error { .. });
        let value = serde_json::to_value(reply)
            .map_err(|_| ErrorData::internal_error("Content result unavailable", None))?;
        Ok(if error {
            CallToolResult::structured_error(value)
        } else {
            CallToolResult::structured(value)
        }
        .into())
    }
    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        self.authorize(&context).await?;
        no_cursor(request)?;
        // Notes are dynamic and vault-scoped: discover through list_notes, then
        // expand the template. Never enumerate every vault's files here.
        Ok(ListResourcesResult::default()
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private))
    }
    async fn list_resource_templates(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        self.authorize(&context).await?;
        no_cursor(request)?;
        Ok(ListResourceTemplatesResult::with_all_items(vec![
            ResourceTemplate::new(resources::TEMPLATE, "note")
                .with_description(
                    "Saved note in a credential-admitted vault; percent-encode the relative path.",
                )
                .with_mime_type("text/markdown"),
        ])
        .with_ttl_ms(0)
        .with_cache_scope(CacheScope::Private))
    }
    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        self.authorize(&context).await?;
        let reply = self
            .execute(&context, resources::command(&request.uri)?)
            .await?;
        match reply.outcome {
            ContentOutcome::Ok {
                value: ContentValue::Note { text, version, .. },
            } => {
                let meta = json!({"io.lapismd/contentVersion":version, "io.lapismd/requestId":reply.request_id});
                Ok(ReadResourceResult::new(vec![
                    ResourceContents::text(text, request.uri)
                        .with_mime_type("text/markdown")
                        .with_meta(MetaObject(meta.as_object().unwrap().clone())),
                ])
                .with_ttl_ms(0)
                .with_cache_scope(CacheScope::Private)
                .into())
            }
            ContentOutcome::Error { error } => Err(content_error(error)),
            _ => Err(ErrorData::internal_error("Unexpected content result", None)),
        }
    }
}
fn parts(context: &RequestContext<RoleServer>) -> Result<Parts, ErrorData> {
    context
        .extensions
        .get::<Parts>()
        .cloned()
        .ok_or_else(|| ErrorData::internal_error("HTTP request context required", None))
}
fn no_cursor(request: Option<PaginatedRequestParams>) -> Result<(), ErrorData> {
    if request.is_some_and(|r| r.cursor.is_some()) {
        return Err(ErrorData::invalid_params(
            "This catalog has no continuation cursor",
            None,
        ));
    }
    Ok(())
}
fn content_error(error: ContentError) -> ErrorData {
    ErrorData::new(
        rmcp::model::ErrorCode(-32000),
        "Content request failed",
        Some(json!(error)),
    )
}
