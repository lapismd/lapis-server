use super::{MAX_BYTES, invalid, read, unavailable};
use crate::services::Result;
use serde_json::Value;
use std::path::Path;
use tokenizers::{PostProcessor, Tokenizer as Engine, TruncationDirection, TruncationParams};

pub struct TokenBatch {
    pub rows: usize,
    pub columns: usize,
    pub ids: Vec<i64>,
    pub mask: Vec<i64>,
    pub types: Vec<i64>,
}
pub(super) struct Tokenizer {
    engine: Engine,
    pad: i64,
    left: bool,
    max: usize,
    truncate_left: bool,
}
impl Tokenizer {
    pub fn open(path: &Path, config: &Path) -> Result<Self> {
        let config: Value = serde_json::from_slice(&read(
            config,
            2 * 1024 * 1024,
            "Embedding tokenizer configuration is unavailable",
        )?)
        .map_err(|_| unavailable("Invalid embedding tokenizer configuration"))?;
        let mut engine =
            Engine::from_bytes(read(path, MAX_BYTES, "Embedding tokenizer is unavailable")?)
                .map_err(|_| unavailable("Invalid embedding tokenizer"))?;
        let max = config
            .get("model_max_length")
            .and_then(Value::as_u64)
            .unwrap_or(512);
        if !(4..=8192).contains(&max) {
            return Err(unavailable(
                "Embedding tokenizer length exceeds native bounds",
            ));
        }
        let direction = if config["truncation_side"] == "left" {
            TruncationDirection::Left
        } else {
            TruncationDirection::Right
        };
        // Transformers.js truncates after special-token processing. Retain
        // enough raw tokens before processing, then cut the completed row.
        let added = engine
            .get_post_processor()
            .map_or(0, |processor| processor.added_tokens(false));
        let input_max = (max as usize)
            .checked_add(added)
            .filter(|n| *n <= 16384)
            .ok_or_else(|| unavailable("Embedding special tokens exceed native bounds"))?;
        engine
            .with_truncation(Some(TruncationParams {
                max_length: input_max,
                direction,
                ..Default::default()
            }))
            .map_err(|_| unavailable("Invalid embedding tokenizer truncation"))?;
        engine.with_padding(None);
        let pad = config["pad_token"]
            .as_str()
            .or_else(|| config["pad_token"]["content"].as_str())
            .unwrap_or("[PAD]");
        let pad = engine
            .token_to_id(pad)
            .ok_or_else(|| unavailable("Embedding tokenizer has no padding token"))?
            as i64;
        Ok(Self {
            engine,
            pad,
            left: config["padding_side"] == "left",
            max: max as usize,
            truncate_left: config["truncation_side"] == "left",
        })
    }
    pub fn encode(&self, texts: &[String]) -> Result<TokenBatch> {
        if texts.len() > 8 || texts.iter().map(String::len).sum::<usize>() > MAX_BYTES {
            return Err(invalid("Embedding token batch exceeds bounds"));
        }
        let rows = texts
            .iter()
            .map(|text| {
                self.engine
                    .encode(text.as_str(), true)
                    .map_err(|_| invalid("Embedding tokenization failed"))
            })
            .collect::<Result<Vec<_>>>()?;
        let columns = rows
            .iter()
            .map(|row| row.get_ids().len())
            .max()
            .unwrap_or(0)
            .min(self.max);
        let size = rows
            .len()
            .checked_mul(columns)
            .ok_or_else(|| invalid("Embedding token batch exceeds bounds"))?;
        let mut batch = TokenBatch {
            rows: rows.len(),
            columns,
            ids: vec![self.pad; size],
            mask: vec![0; size],
            types: vec![0; size],
        };
        for (index, row) in rows.iter().enumerate() {
            let length = row.get_ids().len().min(columns);
            let start = if self.truncate_left {
                row.get_ids().len() - length
            } else {
                0
            };
            let offset = index * columns + if self.left { columns - length } else { 0 };
            for (target, source) in [
                (&mut batch.ids, row.get_ids()),
                (&mut batch.mask, row.get_attention_mask()),
                (&mut batch.types, row.get_type_ids()),
            ] {
                for (i, value) in source[start..start + length].iter().enumerate() {
                    target[offset + i] = *value as i64;
                }
            }
        }
        Ok(batch)
    }
}
