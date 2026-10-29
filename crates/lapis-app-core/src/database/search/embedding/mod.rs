mod config;
mod document;
mod fingerprint;
mod runtime;
use crate::embedding::{OnnxEncoder, host::NativeEmbeddingHost};
use lapis_app_protocol::database::search::embedding::{
    SearchEmbeddingProviderConfig, SearchEmbeddingRuntimeStatus,
};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

pub(in crate::database) struct Provider {
    pub config: Option<SearchEmbeddingProviderConfig>,
    pub status: watch::Sender<Option<SearchEmbeddingRuntimeStatus>>,
    host: NativeEmbeddingHost,
    encoder: Arc<Mutex<Option<OnnxEncoder>>>,
}
pub(in crate::database) use config::validate;

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}
