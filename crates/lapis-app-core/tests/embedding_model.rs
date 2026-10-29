#[path = "support/embedding_reference.rs"]
mod embedding_reference;

use lapis_app_core::embedding::OnnxEncoder;
use serde_json::Value;
use std::path::PathBuf;

#[test]
#[ignore = "Requires the qualified native runtime/model assets; run pnpm test:search:onnx"]
fn real_minilm_inference_matches_retained_tokenization_and_cpu_vectors() {
    let runtime =
        PathBuf::from(std::env::var_os("LAPIS_TEST_ONNX_RUNTIME").expect("qualified runtime path"));
    let models = PathBuf::from(
        std::env::var_os("LAPIS_TEST_ONNX_MODEL").expect("qualified model directory"),
    );
    let fixture = embedding_reference::qualified_fixture();
    let mut encoder = OnnxEncoder::open(
        &runtime,
        &models.join("onnx/model_quantized.onnx"),
        &models.join("tokenizer.json"),
        &models.join("tokenizer_config.json"),
        true,
    )
    .unwrap();
    let mut texts = vec![];
    for row in fixture["queries"].as_array().unwrap() {
        let text = row["text"].as_str().unwrap().to_owned();
        let tokenized = encoder.tokenize(std::slice::from_ref(&text)).unwrap();
        assert_eq!(
            serde_json::to_value(&tokenized.ids).unwrap(),
            row["inputIds"]
        );
        assert_eq!(
            serde_json::to_value(&tokenized.mask).unwrap(),
            row["attentionMask"]
        );
        assert_eq!(
            serde_json::to_value(&tokenized.types).unwrap(),
            row["typeIds"]
        );
        let vectors = encoder.embed(std::slice::from_ref(&text)).unwrap();
        assert_vector(&vectors[0], &row["vector"], true);
        texts.push(text);
    }
    let vectors = encoder.embed(&texts).unwrap();
    for (vector, chunk) in vectors.iter().zip(fixture["chunks"].as_array().unwrap()) {
        assert_vector(vector, &chunk["vector"], true);
    }
    assert_eq!(vectors.len(), texts.len());
    assert!(encoder.embed(&[]).unwrap().is_empty());
    let similarity = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    assert!(similarity(&vectors[0], &vectors[1]) > similarity(&vectors[0], &vectors[2]));

    let mixed = &fixture["mixed"];
    let texts: Vec<String> = serde_json::from_value(mixed["texts"].clone()).unwrap();
    let batch = encoder.tokenize(&texts[..8]).unwrap();
    assert_eq!(batch.rows, 8);
    assert_eq!(batch.columns, 512);
    for (actual, expected) in [
        (&batch.ids, &mixed["inputIds"]),
        (&batch.mask, &mixed["attentionMask"]),
        (&batch.types, &mixed["typeIds"]),
    ] {
        assert_eq!(&serde_json::to_value(actual).unwrap(), expected);
    }
    let vectors = encoder.embed(&texts).unwrap();
    assert_eq!(vectors.len(), 9);
    for (vector, expected) in vectors.iter().zip(mixed["vectors"].as_array().unwrap()) {
        assert_vector(vector, expected, true);
    }
    assert!(encoder.tokenize(&texts).is_err());
    let mut raw = OnnxEncoder::open(
        &runtime,
        &models.join("onnx/model_quantized.onnx"),
        &models.join("tokenizer.json"),
        &models.join("tokenizer_config.json"),
        false,
    )
    .unwrap();
    let vectors = raw
        .embed(&[fixture["raw"]["text"].as_str().unwrap().to_owned()])
        .unwrap();
    assert_vector(&vectors[0], &fixture["raw"]["vector"], false);
    drop(raw);
    drop(encoder);
    let mut reopened = OnnxEncoder::open(
        &runtime,
        &models.join("onnx/model_quantized.onnx"),
        &models.join("tokenizer.json"),
        &models.join("tokenizer_config.json"),
        true,
    )
    .unwrap();
    let vectors = reopened
        .embed(&[fixture["raw"]["text"].as_str().unwrap().to_owned()])
        .unwrap();
    assert_vector(&vectors[0], &fixture["queries"][0]["vector"], true);
    let foreign = tempfile::NamedTempFile::new().unwrap();
    let error = OnnxEncoder::open(
        foreign.path(),
        &models.join("onnx/model_quantized.onnx"),
        &models.join("tokenizer.json"),
        &models.join("tokenizer_config.json"),
        true,
    )
    .err()
    .expect("must not change the process runtime");
    assert!(!error.to_string().contains(foreign.path().to_str().unwrap()));
}
fn assert_vector(actual: &[f64], expected: &Value, normalized: bool) {
    let expected = expected.as_array().unwrap();
    assert_eq!(actual.len(), 384);
    assert_eq!(actual.len(), expected.len());
    let error = actual
        .iter()
        .zip(expected)
        .map(|(a, b)| (a - b.as_f64().unwrap()).abs())
        .fold(0.0, f64::max);
    assert!(error < 0.00001, "maximum model-output error {error}");
    let length = actual.iter().map(|value| value * value).sum::<f64>().sqrt();
    if normalized {
        assert!((length - 1.0).abs() < 0.00001);
    } else {
        assert!((length - 1.0).abs() > 0.1);
    }
}
