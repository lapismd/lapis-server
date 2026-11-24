use super::Provider;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::embedding::host::NativeEmbeddingHost;
use crate::{database::invalid, services::Result};
use lapis_app_protocol::database::search::embedding::{
    SearchEmbeddingProviderConfig as Config, SearchEmbeddingProviderKind as Kind,
    SearchEmbeddingRuntimePhase as Phase, SearchEmbeddingRuntimeStatus,
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

pub(in crate::database) fn validate(config: &Option<Config>) -> Result<()> {
    let Some(config) = config else { return Ok(()) };
    #[cfg(any(target_os = "android", target_os = "ios"))]
    if config.kind != Kind::TokenHash {
        return Err(crate::services::Error::new(
            lapis_app_protocol::ErrorCode::Unsupported,
            "This mobile host supports token-hash embeddings only",
        ));
    }
    if config.dimensions.is_some_and(|n| n == 0 || n > 65536)
        || [
            &config.model_id,
            &config.model_version,
            &config.revision,
            &config.dtype,
        ]
        .into_iter()
        .flatten()
        .any(|s| s.len() > 1024 || s.is_empty())
        || (config.kind == Kind::TransformersJs && config.model_id.is_none())
    {
        return Err(invalid("Invalid native embedding configuration"));
    }
    Ok(())
}
impl Provider {
    pub(in crate::database) fn new(
        #[cfg(not(any(target_os = "android", target_os = "ios")))] host: NativeEmbeddingHost,
    ) -> Self {
        Self {
            config: None,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            host,
            status: watch::channel(None).0,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            encoder: Arc::new(Mutex::new(None)),
        }
    }
    pub(in crate::database) fn reset(&mut self, config: Option<Config>) {
        self.config = config;
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            self.encoder = Arc::new(Mutex::new(None));
        }
        self.status.send_replace(self.config.as_ref().map(|config| {
            SearchEmbeddingRuntimeStatus {
                provider_kind: config.kind,
                phase: if config.kind == Kind::TokenHash {
                    Phase::Ready
                } else {
                    Phase::Downloading
                },
                model_id: config
                    .model_id
                    .clone()
                    .or_else(|| Some("lapis/token-hash-v0".into())),
                message: Some("Waiting to initialize embedding runtime".into()),
                progress: Some(0.0),
                updated_at: super::now(),
            }
        }));
    }
}
