use lapis_app_protocol::operations::OperationDescriptor;
use rmcp::{RoleServer, model::*, service::RequestContext};
use serde_json::json;
use std::{collections::BTreeMap, io, sync::Arc};

pub const MIME_TYPE: &str = "text/html;profile=mcp-app";
pub const MAX_HTML_BYTES: usize = 8 * 1024 * 1024;
const EXTENSION: &str = "io.modelcontextprotocol/ui";
const ACTIONS_META: &str = "io.lapismd/availableTools";

/// An immutable contribution from the hosted application or a compiled provider.
pub struct AppResource {
    pub uri: String,
    pub title: String,
    pub html: String,
}

pub(crate) struct App {
    resource: AppResource,
    followups: Vec<String>,
}

#[derive(Clone, Default)]
pub struct AppRegistry(BTreeMap<String, Arc<App>>);

impl AppRegistry {
    pub fn register_builtin(&mut self, tool: &str, resource: AppResource) -> io::Result<()> {
        if !crate::catalog::is_builtin(tool) {
            return Err(io::Error::other("Unknown MCP App built-in tool"));
        }
        self.insert(tool.to_owned(), resource, Vec::new())
    }

    /// The host supplies the descriptors of the provider it is registering.
    /// Follow-up names describe presentation availability. Dispatch still checks grants.
    pub fn register_provider(
        &mut self,
        provider: &[OperationDescriptor],
        plugin: &str,
        operation: &str,
        followups: &[&str],
        resource: AppResource,
    ) -> io::Result<()> {
        let contains = |id| {
            provider
                .iter()
                .any(|item| item.plugin_id == plugin && item.id == id)
        };
        if !contains(operation)
            || followups.iter().any(|id| !contains(*id))
            || !resource.uri.starts_with(&format!("ui://lapis/{plugin}/"))
        {
            return Err(io::Error::other("Invalid provider MCP App binding"));
        }
        self.insert(
            format!("{plugin}_{operation}"),
            resource,
            followups
                .iter()
                .map(|id| format!("{plugin}_{id}"))
                .collect(),
        )
    }

    fn insert(
        &mut self,
        tool: String,
        resource: AppResource,
        followups: Vec<String>,
    ) -> io::Result<()> {
        let uri = resource.uri.strip_prefix("ui://lapis/").unwrap_or_default();
        if uri.is_empty()
            || uri.len() > 128
            || uri.split('/').any(|part| {
                part.is_empty()
                    || !part
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            })
            || resource.title.trim().is_empty()
            || resource.html.trim().is_empty()
            || resource.html.len() > MAX_HTML_BYTES
            || self.0.contains_key(&tool)
            || self.0.values().any(|app| app.resource.uri == resource.uri)
        {
            return Err(io::Error::other("Invalid or duplicate MCP App resource"));
        }
        self.0.insert(
            tool,
            Arc::new(App {
                resource,
                followups,
            }),
        );
        Ok(())
    }

    pub(crate) fn get(&self, tool: &str) -> Option<&App> {
        self.0.get(tool).map(Arc::as_ref)
    }

    pub(crate) fn find(&self, uri: &str) -> Option<(&str, &App)> {
        self.0.iter().find_map(|(tool, app)| {
            (app.resource.uri == uri).then_some((tool.as_str(), app.as_ref()))
        })
    }

    pub(crate) fn attach(&self, tools: &mut [Tool]) {
        for tool in tools {
            if let Some(app) = self.get(&tool.name) {
                tool.meta
                    .get_or_insert_with(MetaObject::default)
                    .0
                    .insert("ui".into(), json!({"resourceUri": app.resource.uri}));
            }
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl App {
    pub(crate) fn listing(&self) -> Resource {
        Resource::new(self.resource.uri.clone(), self.resource.title.clone())
            .with_mime_type(MIME_TYPE)
            .with_meta(resource_meta())
    }

    pub(crate) fn content(&self) -> ResourceContents {
        ResourceContents::text(self.resource.html.clone(), self.resource.uri.clone())
            .with_mime_type(MIME_TYPE)
            .with_meta(resource_meta())
    }

    pub(crate) fn tool_meta(&self, operations: &[OperationDescriptor]) -> MetaObject {
        let tools = self
            .followups
            .iter()
            .filter(|name| {
                operations
                    .iter()
                    .any(|item| **name == format!("{}_{}", item.plugin_id, item.id))
            })
            .collect::<Vec<_>>();
        MetaObject([(ACTIONS_META.into(), json!(tools))].into_iter().collect())
    }
}

pub(crate) fn supported(context: &RequestContext<RoleServer>) -> bool {
    // Legacy stateless HTTP retains no initialize capabilities between requests.
    // UI metadata is optional for those clients and the host still negotiates with the view.
    context
        .meta
        .client_capabilities()
        .is_none_or(|capabilities| {
            capabilities
                .extensions
                .and_then(|mut extensions| extensions.remove(EXTENSION))
                .and_then(|mut settings| settings.remove("mimeTypes"))
                .and_then(|types| {
                    types
                        .as_array()
                        .map(|types| types.iter().any(|value| value == MIME_TYPE))
                })
                .unwrap_or(false)
        })
}

fn resource_meta() -> MetaObject {
    MetaObject([("ui".into(), json!({
        "csp": {"connectDomains": [], "resourceDomains": [], "frameDomains": [], "baseUriDomains": []},
        "permissions": {}
    }))].into_iter().collect())
}
