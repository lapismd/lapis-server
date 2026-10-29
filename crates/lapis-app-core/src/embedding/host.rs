use super::{OnnxEncoder, unavailable};
use crate::services::Result;
use lapis_app_protocol::database::search::embedding::SearchEmbeddingProviderConfig;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Registered by native installation/cache policy, never deserialized from IPC.
#[derive(Clone)]
pub struct OnnxModelAssets {
    pub model_id: String,
    pub revision: String,
    pub dtype: String,
    pub model: PathBuf,
    pub tokenizer: PathBuf,
    pub tokenizer_config: PathBuf,
}
#[derive(Clone, Default)]
pub struct NativeEmbeddingHost {
    runtime: Option<PathBuf>,
    models: Arc<Vec<OnnxModelAssets>>,
    cache: Option<Arc<super::cache::Cache>>,
}
impl NativeEmbeddingHost {
    pub fn new(runtime: PathBuf, models: Vec<OnnxModelAssets>) -> Self {
        Self {
            runtime: Some(runtime),
            models: Arc::new(models),
            cache: None,
        }
    }
    /// Native installation policy admits an existing cache outside vaults.
    /// Asset preparation stays lazy and runs inside the accepted worker.
    pub fn managed(directory: &Path) -> Result<Self> {
        Ok(Self {
            cache: Some(Arc::new(super::cache::Cache::open(directory)?)),
            ..Self::default()
        })
    }
    pub(crate) fn open(
        &self,
        config: &SearchEmbeddingProviderConfig,
        progress: &mut dyn FnMut(f64),
    ) -> Result<OnnxEncoder> {
        let prepared = self
            .cache
            .as_ref()
            .map(|cache| cache.prepare(config, progress))
            .transpose()?;
        let runtime = prepared
            .as_ref()
            .map(|value| &value.runtime)
            .or(self.runtime.as_ref())
            .ok_or_else(|| unavailable("Native ONNX runtime is not admitted"))?;
        let model = prepared
            .as_ref()
            .map(|value| &value.model)
            .or_else(|| {
                self.models.iter().find(|model| {
                    Some(model.model_id.as_str()) == config.model_id.as_deref()
                        && config
                            .revision
                            .as_ref()
                            .is_none_or(|revision| revision == &model.revision)
                        && config.dtype.as_deref().unwrap_or("q8") == model.dtype
                })
            })
            .ok_or_else(|| unavailable("Native embedding model is not admitted"))?;
        OnnxEncoder::open(
            runtime,
            &model.model,
            &model.tokenizer,
            &model.tokenizer_config,
            config.normalize.unwrap_or(true),
        )
    }
}
