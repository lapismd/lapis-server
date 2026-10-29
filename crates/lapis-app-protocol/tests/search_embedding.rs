use lapis_app_protocol::database::search::embedding::SearchEmbeddingProviderConfig as Config;
use serde_json::json;

#[test]
fn portable_embedding_configuration_roundtrips_without_native_authority() {
    let config = json!({"kind":"transformers-js","modelId":"Xenova/all-MiniLM-L6-v2","revision":"revision","modelVersion":"version","dtype":"q8","dimensions":384,"device":"wasm","pooling":"mean","normalize":true,"allowRemoteModels":false});
    let parsed: Config = serde_json::from_value(config.clone()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), config);
    for field in [
        "runtime",
        "root",
        "localModelPath",
        "wasmPaths",
        "executable",
        "url",
    ] {
        let mut invalid = config.clone();
        invalid[field] = json!("/not-client-authority");
        assert!(
            serde_json::from_value::<Config>(invalid).is_err(),
            "{field}"
        );
    }
    for (field, value) in [
        ("dimensions", json!(-1)),
        ("dimensions", json!(1.5)),
        ("kind", json!("native")),
        ("pooling", json!("max")),
    ] {
        let mut invalid = config.clone();
        invalid[field] = value;
        assert!(
            serde_json::from_value::<Config>(invalid).is_err(),
            "{field}"
        );
    }
}
