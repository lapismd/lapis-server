pub mod archive;
pub mod cache;
pub mod config;
pub mod credentials;
pub mod install;
pub mod links;
pub mod registry;
pub mod runtime;

use sha2::{Digest, Sha256};
pub fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
pub fn log(event: &str, detail: &str) {
    eprintln!(
        "{}",
        serde_json::json!({"component":"mod-loader", "event":event, "detail":detail})
    );
}
