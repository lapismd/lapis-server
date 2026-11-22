use crate::{AppRegistry, apps, catalog::Catalog, resources};
use axum::http::request::Parts;
use lapis_app_protocol::{content::*, operations::OperationCatalog};
use lapis_content_http::Boundary;
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use serde_json::json;
use std::{borrow::Cow, sync::Arc};
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct Handler {
    pub boundary: Boundary,
    pub catalog: Arc<Catalog>,
    pub initialized_clients: bool,
    pub apps: Arc<AppRegistry>,
}
impl Handler {
    async fn authorize(&self, context: &RequestContext<RoleServer>) -> Result<(), ErrorData> {
        self.boundary
            .check(parts(context)?)
            .await
            .map_err(|e| content_error(e.into_error()))?;
        Ok(())
    }
    async fn operation_catalog(
        &self,
        context: &RequestContext<RoleServer>,
        vault: Option<Uuid>,
    ) -> Result<OperationCatalog, ErrorData> {
        self.boundary
            .operation_catalog(parts(context)?, vault)
            .await
            .map_err(|error| content_error(error.into_error()))
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
        let mut info = Implementation::new("lapis-content", env!("CARGO_PKG_VERSION"));
        if self.initialized_clients {
            info = info.with_icons(vec![
                Icon::new(format!("{}/icon.png", self.boundary.origin()))
                    .with_mime_type("image/png")
                    .with_sizes(vec!["512x512".to_owned()]),
            ]);
        }
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(info)
    }
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        if self.initialized_clients {
            Cow::Borrowed(&[
                ProtocolVersion::V_2026_07_28,
                ProtocolVersion::V_2025_11_25,
                ProtocolVersion::V_2025_06_18,
                ProtocolVersion::V_2025_03_26,
            ])
        } else {
            Cow::Borrowed(&[ProtocolVersion::V_2026_07_28])
        }
    }
    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        self.authorize(&context).await?;
        no_cursor(request)?;
        let mut tools = self.catalog.tools.clone();
        tools.extend(crate::operations::tools(
            self.operation_catalog(&context, None).await?,
        )?);
        if apps::supported(&context) {
            self.apps.attach(&mut tools);
        }
        Ok(ListToolsResult::with_all_items(tools)
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.authorize(&context).await?;
        if !self
            .catalog
            .tools
            .iter()
            .any(|tool| tool.name == request.name)
        {
            let catalog = self.operation_catalog(&context, None).await?;
            if let Some(descriptor) = catalog
                .operations
                .iter()
                .find(|d| crate::operations::tool_name(d) == request.name)
            {
                let plugin = descriptor.plugin_id.clone();
                let operation = descriptor.id.clone();
                let mut arguments = request.arguments.unwrap_or_default();
                let vault = arguments
                    .remove("vault_id")
                    .and_then(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                    .ok_or_else(|| ErrorData::invalid_params("Explicit vault_id required", None))?;
                let request_id = match arguments.remove("invocation_id") {
                    Some(value) => value
                        .as_str()
                        .and_then(|s| Uuid::parse_str(s).ok())
                        .ok_or_else(|| {
                            ErrorData::invalid_params("invocation_id must be a UUID", None)
                        })?,
                    None if descriptor.effect
                        == lapis_app_protocol::content::ContentEffect::Write =>
                    {
                        return Err(ErrorData::invalid_params(
                            "Writes require a stable invocation_id",
                            None,
                        ));
                    }
                    None => Uuid::new_v4(),
                };
                let input = lapis_app_protocol::operations::OperationRequest {
                    protocol_version: 1,
                    request_id,
                    version: descriptor.version,
                    input: serde_json::Value::Object(arguments),
                };
                let reply = self
                    .boundary
                    .invoke_operation(parts(&context)?, vault, plugin, operation, input)
                    .await
                    .map_err(|e| content_error(e.into_error()))?;
                let error = matches!(
                    reply.outcome,
                    lapis_app_protocol::operations::OperationOutcome::Error { .. }
                );
                let value = serde_json::to_value(reply)
                    .map_err(|_| ErrorData::internal_error("Operation result unavailable", None))?;
                if error {
                    return Ok(CallToolResult::structured_error(value).into());
                }
                let mut result = CallToolResult::structured(value);
                if apps::supported(&context)
                    && let Some(app) = self.apps.get(&request.name)
                    && let Ok(catalog) = self.operation_catalog(&context, Some(vault)).await
                {
                    result.meta = Some(app.tool_meta(&catalog.operations));
                }
                return Ok(result.into());
            }
        }
        let command = self.catalog.command(&request.name, request.arguments)?;
        let reply = self.execute(&context, command).await?;
        let error = matches!(reply.outcome, ContentOutcome::Error { .. });
        let value = if matches!(
            &reply.outcome,
            ContentOutcome::Ok {
                value: ContentValue::Capabilities { .. }
            }
        ) {
            let registered = self.operation_catalog(&context, None).await?;
            serde_json::to_value(crate::capabilities::Reply::new(
                reply,
                &self.catalog.tools,
                registered,
            )?)
        } else {
            serde_json::to_value(reply)
        }
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
        // Saved notes still use the dynamic template. Only packaged views are listed here.
        let mut resources = Vec::new();
        if !self.apps.is_empty() && apps::supported(&context) {
            let mut tools = self.catalog.tools.clone();
            tools.extend(crate::operations::tools(
                self.operation_catalog(&context, None).await?,
            )?);
            resources.extend(
                tools
                    .iter()
                    .filter_map(|tool| self.apps.get(&tool.name).map(|app| app.listing())),
            );
        }
        Ok(ListResourcesResult::with_all_items(resources)
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
        if request.uri.starts_with("ui://") {
            let (tool, app) = self.apps.find(&request.uri).ok_or_else(app_not_found)?;
            if !crate::catalog::is_builtin(tool) {
                let catalog = self.operation_catalog(&context, None).await?;
                if !catalog
                    .operations
                    .iter()
                    .any(|item| crate::operations::tool_name(item) == tool)
                {
                    return Err(app_not_found());
                }
            }
            return Ok(ReadResourceResult::new(vec![app.content()])
                .with_ttl_ms(0)
                .with_cache_scope(CacheScope::Private)
                .into());
        }
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
fn app_not_found() -> ErrorData {
    ErrorData::resource_not_found("MCP App resource not found", None)
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
