//! Metadata emitted by the same declarations that register each HTTP operation.
use schemars::{JsonSchema, SchemaGenerator};
use serde_json::{Value, json};

#[doc(hidden)]
pub use inventory as register;

#[derive(Clone, Copy)]
pub enum Auth {
    Public,
    Login,
    Owner,
    Browser,
    BrowserNavigation,
    Content,
    Mcp,
    OAuthClient,
    Sync,
}

#[derive(Clone, Copy)]
pub struct Body {
    pub media: &'static str,
    pub schema: fn(&mut SchemaGenerator) -> Value,
}

pub fn schema<T: JsonSchema>(generator: &mut SchemaGenerator) -> Value {
    generator.subschema_for::<T>().to_value()
}

pub const fn json_body<T: JsonSchema>() -> Option<Body> {
    Some(Body {
        media: "application/json",
        schema: schema::<T>,
    })
}

pub const fn form_body<T: JsonSchema>() -> Option<Body> {
    Some(Body {
        media: "application/x-www-form-urlencoded",
        schema: schema::<T>,
    })
}

pub const fn bytes(media: &'static str) -> Option<Body> {
    Some(Body {
        media,
        schema: |_| json!({"type":"string","format":"binary"}),
    })
}

pub const fn text(media: &'static str) -> Option<Body> {
    Some(Body {
        media,
        schema: |_| json!({"type":"string"}),
    })
}

pub struct Endpoint {
    pub path: &'static str,
    pub method: &'static str,
    pub auth: Auth,
    pub summary: &'static str,
    pub success: u16,
    pub input: Option<Body>,
    pub output: Option<Body>,
}
inventory::collect!(Endpoint);

/// Define the Axum extension without making the portable crate depend on Axum.
#[macro_export]
macro_rules! api_router_extension {
    () => {
        trait RegisterRoute: Sized {
            type Method;
            fn api(self, route: (&'static str, Self::Method)) -> Self;
        }
        impl<S: Clone + Send + Sync + 'static> RegisterRoute for ::axum::Router<S> {
            type Method = ::axum::routing::MethodRouter<S>;
            fn api(self, (path, method): (&'static str, Self::Method)) -> Self {
                self.route(path, method)
            }
        }
    };
}

/// Runtime path/method and public documentation are registered together.
#[macro_export]
macro_rules! api_route {
    ($path:literal, $auth:ident, {
        $($method:ident($handler:path) => ($summary:literal, $status:literal, $input:expr, $output:expr)),+ $(,)?
    } $(, layer = $layer:expr)? ) => {{
        $(
            $crate::api_operation!($path, $auth, stringify!($method), $summary, $status, $input, $output);
        )+
        (
            $path,
            ::axum::routing::MethodRouter::new()$(.$method($handler))+$(.layer($layer))?,
        )
    }};
}

/// Use for transports registered as a service, and for paths dispatched by a shared fallback.
#[macro_export]
macro_rules! api_operation {
    ($path:expr, $auth:ident, $method:expr, $summary:literal, $status:literal, $input:expr, $output:expr) => {
        $crate::http_docs::register::submit! {
            $crate::http_docs::Endpoint {
                path: $path,
                method: $method,
                auth: $crate::http_docs::Auth::$auth,
                summary: $summary,
                success: $status,
                input: $input,
                output: $output,
            }
        }
    };
}
