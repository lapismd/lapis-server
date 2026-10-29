mod catalog;
mod install;
mod remote;

use super::{host::OnnxModelAssets, unavailable};
use crate::{services::Result, vault::VaultRoot};
use lapis_app_protocol::database::search::embedding::SearchEmbeddingProviderConfig;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) struct Cache {
    root: VaultRoot,
}
pub(super) struct Prepared {
    pub runtime: PathBuf,
    pub model: OnnxModelAssets,
}
impl Cache {
    pub fn open(directory: &Path) -> Result<Self> {
        if !directory.is_absolute()
            || !std::fs::symlink_metadata(directory).is_ok_and(|metadata| metadata.is_dir())
        {
            return Err(unavailable(
                "Native embedding cache needs an existing physical directory",
            ));
        }
        Ok(Self {
            root: VaultRoot::open(directory)?,
        })
    }

    pub fn prepare(
        &self,
        config: &SearchEmbeddingProviderConfig,
        progress: &mut dyn FnMut(f64),
    ) -> Result<Prepared> {
        catalog::select(config)?;
        let runtime = catalog::runtime_asset()?;
        let models = catalog::model_assets();
        let assets: Vec<_> = std::iter::once(runtime.clone())
            .chain(models.iter().cloned())
            .collect();
        self.install(
            &assets,
            config.allow_remote_models.unwrap_or(true),
            &mut remote::Remote::default(),
            progress,
        )?;
        let model_path = |name: &str| {
            self.root.path().join(
                models
                    .iter()
                    .find(|asset| asset.name == name)
                    .unwrap()
                    .filename(),
            )
        };
        self.root.verify_physical_root()?;
        Ok(Prepared {
            runtime: self.root.path().join(runtime.filename()),
            model: OnnxModelAssets {
                model_id: catalog::MODEL.into(),
                revision: catalog::REVISION.into(),
                dtype: "q8".into(),
                model: model_path("model_quantized.onnx"),
                tokenizer: model_path("tokenizer.json"),
                tokenizer_config: model_path("tokenizer_config.json"),
            },
        })
    }

    fn install(
        &self,
        assets: &[catalog::Asset],
        remote: bool,
        fetch: &mut impl remote::Fetch,
        progress: &mut dyn FnMut(f64),
    ) -> Result<()> {
        progress(0.0);
        let _lease = self.lease(Duration::from_secs(180))?;
        let total = assets.iter().map(|asset| asset.bytes).sum::<u64>().max(1);
        let mut completed = 0;
        for asset in assets {
            self.ensure(asset, remote, fetch, &mut |bytes| {
                progress((completed + bytes) as f64 / total as f64)
            })?;
            completed += asset.bytes;
            progress(completed as f64 / total as f64);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
