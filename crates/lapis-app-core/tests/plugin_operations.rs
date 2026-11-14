use lapis_app_core::services::{
    Client, Runtime,
    content::{ContentGrants, ContentVault},
    plugin_operations::*,
};
use lapis_app_protocol::{content::*, operations::*};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

struct Echo {
    plugin: &'static str,
    enabled: Arc<AtomicBool>,
}
impl OperationProvider for Echo {
    fn descriptors(&self) -> Vec<OperationDescriptor> {
        vec![OperationDescriptor {
            plugin_id: self.plugin.into(),
            id: "echo".into(),
            version: 1,
            description: "Echo a message".into(),
            permission: format!("{}:read", self.plugin),
            effect: ContentEffect::ReadOnly,
            input_schema: json!({"type":"object", "properties":{"message":{"type":"string"}}, "required":["message"], "additionalProperties":false}),
            output_schema: json!({"type":"string"}),
            help: OperationHelp {
                command: vec![self.plugin.into(), "echo".into()],
                summary: "Echo".into(),
                markdown: "Echo a message.".into(),
                examples: vec![],
                keywords: vec![],
                arguments: vec![OperationArgument {
                    flag: "message".into(),
                    field: "message".into(),
                    description: "Message".into(),
                    required: true,
                    json: false,
                }],
            },
        }]
    }
    fn available(&self, _: &Client) -> Result<bool, ContentError> {
        Ok(self.enabled.load(Ordering::Acquire))
    }
    fn invoke(
        &self,
        _: &Client,
        _: &str,
        request: &OperationRequest,
    ) -> Result<Value, ContentError> {
        Ok(request.input["message"].clone())
    }
}

#[test]
fn multiple_providers_share_scoped_dispatch_and_disable() {
    let enabled = Arc::new(AtomicBool::new(true));
    let mut registry = OperationRegistry::default();
    for plugin in ["alpha", "beta"] {
        registry
            .register(Arc::new(Echo {
                plugin,
                enabled: enabled.clone(),
            }))
            .unwrap();
    }
    assert!(
        registry
            .register(Arc::new(Echo {
                plugin: "alpha",
                enabled: enabled.clone()
            }))
            .is_err()
    );
    let grants = ContentGrants::with_operations(Arc::new(registry));
    grants.set_enabled(true).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let owner = runtime.admit(id, directory.path()).unwrap();
    let issue = |permissions: &[String]| {
        grants
            .issue_named_operations(
                "test",
                vec![ContentVault {
                    name: "test".into(),
                    client: owner.attach().unwrap(),
                }],
                Duration::from_secs(60),
                permissions,
            )
            .unwrap()
    };
    let old = issue(&[]);
    assert!(
        grants
            .operation_catalog(old.token.expose_secret(), id)
            .unwrap()
            .operations
            .is_empty()
    );
    let scoped = issue(&["alpha:read".into()]);
    let bearer = scoped.token.expose_secret();
    assert_eq!(
        grants
            .operation_catalog(bearer, id)
            .unwrap()
            .operations
            .len(),
        1
    );
    assert!(grants.operation_catalog(bearer, Uuid::new_v4()).is_err());
    let mut request = OperationRequest {
        protocol_version: 1,
        request_id: Uuid::new_v4(),
        version: 1,
        input: json!({"message":"hello"}),
    };
    assert!(
        matches!(grants.invoke_operation(bearer, id, "alpha", "echo", request.clone()).outcome, OperationOutcome::Ok {value} if value == "hello")
    );
    assert!(
        matches!(grants.invoke_operation(bearer, id, "beta", "echo", request.clone()).outcome, OperationOutcome::Error {error} if error.code == ContentErrorCode::PermissionDenied)
    );
    request.input = json!({"message":12});
    assert!(
        matches!(grants.invoke_operation(bearer, id, "alpha", "echo", request.clone()).outcome, OperationOutcome::Error {error} if error.code == ContentErrorCode::InvalidRequest)
    );
    request.input = json!({"message":"ok"});
    request.version = 2;
    assert!(
        matches!(grants.invoke_operation(bearer, id, "alpha", "echo", request.clone()).outcome, OperationOutcome::Error {error} if error.code == ContentErrorCode::ProtocolMismatch)
    );
    enabled.store(false, Ordering::Release);
    assert!(
        grants
            .operation_catalog(bearer, id)
            .unwrap()
            .operations
            .is_empty()
    );
    request.version = 1;
    assert!(
        matches!(grants.invoke_operation(bearer, id, "alpha", "echo", request.clone()).outcome, OperationOutcome::Error {error} if error.code == ContentErrorCode::Unavailable)
    );
    grants.revoke(scoped.grant_id).unwrap();
    assert!(grants.operation_catalog(bearer, id).is_err());
}

#[test]
fn registry_rejects_reserved_roots() {
    let mut registry = OperationRegistry::default();
    assert!(
        registry
            .register(Arc::new(Echo {
                plugin: "help",
                enabled: Arc::new(AtomicBool::new(true))
            }))
            .is_err()
    );
}
