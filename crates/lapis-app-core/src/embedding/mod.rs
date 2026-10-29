//! CPU inference only. Native hosts admit these files; paths are never DTOs.
//! Call encoding/inference from an owned blocking worker, not a UI executor.
mod cache;
pub mod host;
mod pooling;
mod runtime;
mod tokenization;

use crate::services::{Error, Result};
use lapis_app_protocol::ErrorCode;
use ort::{
    environment::Environment,
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use std::{fs::File, io::Read, path::Path};
pub use tokenization::TokenBatch;

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_VALUES: usize = MAX_BYTES / 8;

pub struct OnnxEncoder {
    session: Session,
    tokenizer: tokenization::Tokenizer,
    _environment: Environment,
    normalize: bool,
}
impl OnnxEncoder {
    /// All four paths are native installation/data authority, not configuration
    /// supplied by a renderer or an HTTP caller. The model is read into memory
    /// so loading it never supplies an implicit directory for external tensors.
    pub fn open(
        runtime: &Path,
        model: &Path,
        tokenizer: &Path,
        config: &Path,
        normalize: bool,
    ) -> Result<Self> {
        if !runtime.is_file() {
            return Err(unavailable("Native ONNX runtime is unavailable"));
        }
        let tokenizer = tokenization::Tokenizer::open(tokenizer, config)?;
        let model = read(
            model,
            512 * 1024 * 1024,
            "Embedding model is unavailable or exceeds bounds",
        )?;
        let environment = runtime::load(runtime)?;
        let session = Session::builder(&environment)
            .map_err(|_| unavailable("Native ONNX session could not start"))?
            .with_intra_threads(1)
            .map_err(|_| unavailable("Native ONNX thread policy is unavailable"))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|_| unavailable("Native ONNX optimization is unavailable"))?
            .commit_from_memory(&model)
            .map_err(|_| unavailable("Embedding model could not be loaded"))?;
        if session.inputs().iter().any(|input| {
            !matches!(
                input.name(),
                "input_ids" | "attention_mask" | "token_type_ids" | "position_ids"
            )
        }) {
            return Err(unavailable("Embedding model requires an unsupported input"));
        }
        Ok(Self {
            session,
            tokenizer,
            _environment: environment,
            normalize,
        })
    }
    pub fn tokenize(&self, texts: &[String]) -> Result<TokenBatch> {
        self.tokenizer.encode(texts)
    }
    pub fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f64>>> {
        if texts.len() > 100_000 || texts.iter().map(String::len).sum::<usize>() > MAX_BYTES {
            return Err(invalid("Embedding input exceeds bounds"));
        }
        let mut result = vec![];
        let mut values = 0usize;
        for texts in texts.chunks(8) {
            let batch = self.tokenize(texts)?;
            let shape = [batch.rows, batch.columns];
            let mut inputs = vec![];
            for input in self.session.inputs() {
                let data = match input.name() {
                    "input_ids" => batch.ids.clone(),
                    "attention_mask" => batch.mask.clone(),
                    "token_type_ids" => batch.types.clone(),
                    "position_ids" => (0..batch.rows)
                        .flat_map(|_| (0..batch.columns).map(|n| n as i64))
                        .collect(),
                    _ => return Err(unavailable("Embedding model requires an unsupported input")),
                };
                inputs.push((
                    input.name().to_owned(),
                    Tensor::from_array((shape, data))
                        .map_err(|_| invalid("Invalid embedding input tensor"))?,
                ));
            }
            let outputs = self
                .session
                .run(inputs)
                .map_err(|_| unavailable("Native embedding inference failed"))?;
            let output = [
                "last_hidden_state",
                "token_embeddings",
                "sentence_embedding",
                "embeddings",
            ]
            .iter()
            .find_map(|name| outputs.get(name))
            .ok_or_else(|| unavailable("Embedding model has no supported output"))?;
            let (shape, data) = output
                .try_extract_tensor::<f32>()
                .map_err(|_| unavailable("Embedding model output must be float32"))?;
            let vectors = pooling::pool(
                shape,
                data,
                &batch.mask,
                batch.rows,
                batch.columns,
                self.normalize,
            )?;
            values = values.saturating_add(vectors.iter().map(Vec::len).sum::<usize>());
            if values > MAX_VALUES {
                return Err(invalid("Embedding output exceeds bounds"));
            }
            result.extend(vectors);
        }
        Ok(result)
    }
}
fn read(path: &Path, limit: usize, message: &'static str) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(|_| unavailable(message))?;
    let metadata = file.metadata().map_err(|_| unavailable(message))?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(unavailable(message));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| unavailable(message))?;
    if bytes.len() > limit {
        return Err(unavailable(message));
    }
    Ok(bytes)
}
fn unavailable(message: &'static str) -> Error {
    Error::new(ErrorCode::Unsupported, message)
}
fn invalid(message: &'static str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}
