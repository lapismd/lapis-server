use super::super::unavailable;
use crate::services::Result;
use lapis_app_protocol::database::search::embedding::SearchEmbeddingProviderConfig;

pub(super) const MODEL: &str = "Xenova/all-MiniLM-L6-v2";
pub(super) const REVISION: &str = "751bff37182d3f1213fa05d7196b954e230abad9";

#[derive(Clone)]
pub(super) struct Asset {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
    pub url: String,
}
impl Asset {
    pub fn filename(&self) -> String {
        format!("{}-{}", self.sha256, self.name)
    }
}

pub(super) fn select(config: &SearchEmbeddingProviderConfig) -> Result<()> {
    if config.model_id.as_deref() != Some(MODEL)
        || config
            .revision
            .as_deref()
            .is_some_and(|revision| revision != REVISION)
        || config.dtype.as_deref().unwrap_or("q8") != "q8"
    {
        return Err(unavailable("Native embedding model is not admitted"));
    }
    Ok(())
}

pub(super) fn model_assets() -> Vec<Asset> {
    [
        (
            "config.json",
            "config.json",
            650,
            "7135149f7cffa1a573466c6e4d8423ed73b62fd2332c575bf738a0d033f70df7",
        ),
        (
            "tokenizer.json",
            "tokenizer.json",
            711661,
            "da0e79933b9ed51798a3ae27893d3c5fa4a201126cef75586296df9b4d2c62a0",
        ),
        (
            "tokenizer_config.json",
            "tokenizer_config.json",
            366,
            "9261e7d79b44c8195c1cada2b453e55b00aeb81e907a6664974b4d7776172ab3",
        ),
        (
            "special_tokens_map.json",
            "special_tokens_map.json",
            125,
            "b6d346be366a7d1d48332dbc9fdf3bf8960b5d879522b7799ddba59e76237ee3",
        ),
        (
            "model_quantized.onnx",
            "onnx/model_quantized.onnx",
            22972370,
            "afdb6f1a0e45b715d0bb9b11772f032c399babd23bfc31fed1c170afc848bdb1",
        ),
    ]
    .into_iter()
    .map(|(name, path, bytes, digest)| Asset {
        name: name.into(),
        bytes,
        sha256: digest.into(),
        url: format!("https://huggingface.co/{MODEL}/resolve/{REVISION}/{path}"),
    })
    .collect()
}

pub(super) fn runtime_asset() -> Result<Asset> {
    let (platform, arch, name, bytes, digest) = match (std::env::consts::OS, std::env::consts::ARCH)
    {
        ("macos", "aarch64") => (
            "darwin",
            "arm64",
            "libonnxruntime.1.30.0.dylib",
            44589928,
            "685d2be5dba1309c89d3a5324b7fd06a5c42f1a61bfea32d14c4cc28d072121d",
        ),
        ("linux", "aarch64") => (
            "linux",
            "arm64",
            "libonnxruntime.so.1",
            25135496,
            "1f549d46250b005580b597f4164984aaf75b3bcb9f3aeb07c7f8a8fe60b23c76",
        ),
        ("linux", "x86_64") => (
            "linux",
            "x64",
            "libonnxruntime.so.1",
            45828512,
            "ffb75a925ba05e47b235bb66e3e3911714a80b328a9c9425539feb204aa32a23",
        ),
        _ => {
            return Err(unavailable(
                "Native ONNX runtime is not admitted for this platform",
            ));
        }
    };
    Ok(Asset {
        name: name.into(),
        bytes,
        sha256: digest.into(),
        url: format!(
            "https://unpkg.com/onnxruntime-node@1.30.0/bin/napi-v6/{platform}/{arch}/{name}"
        ),
    })
}
