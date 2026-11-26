use lapis_plugin_jobs::{ExecutionLimits, JobHost, JobPermissions, execute};
use serde_json::{Value, json};
use std::sync::Arc;

#[test]
fn explicit_network_declarations_preserve_v1_default_denial() {
    let legacy = json!({"metadataRead":false,"frontmatterRead":false,
        "notifications":false,"frontmatterFields":[]});
    let permissions: JobPermissions = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(serde_json::to_value(permissions).unwrap(), legacy);
    let mut declared = legacy;
    declared["network"] = json!([{"origin":"https://api.example.com",
        "methods":["GET"],"allowPrivateNetwork":false,
        "credentials":[{"id":"bank","header":"authorization","prefix":"Bearer "}]}]);
    let permissions: JobPermissions = serde_json::from_value(declared.clone()).unwrap();
    assert_eq!(serde_json::to_value(permissions).unwrap(), declared);
}

struct Host;
impl JobHost for Host {
    fn invoke(&self, action: &str, input: Value) -> Result<Value, String> {
        assert_eq!(action, "network.request");
        assert_eq!(
            input,
            json!({"url":"https://api.example.com/transactions","credential":"bank"})
        );
        Ok(json!({"status":200,"body":"[]","headers":{}}))
    }
    fn cancelled(&self) -> bool {
        false
    }
}

#[test]
fn network_is_a_native_broker_call_without_ambient_fetch_or_secret_reads() {
    execute(
        "export function run(ctx) { if(typeof fetch!=='undefined'||ctx.credentials) throw Error('ambient authority'); const r=ctx.network.request({url:'https://api.example.com/transactions',credential:'bank'}); if(r.status!==200||r.body!=='[]') throw Error('response'); }",
        &json!({}), Arc::new(Host), ExecutionLimits::default(),
    ).unwrap();
}
