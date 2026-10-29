use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};

const LEGACY: &str =
    include_str!("../../../../packages/api/src/lib/__tests__/fixtures/search-semantic-model.json");
const LINUX: &str =
    include_str!("../fixtures/search-model/linux-x64-ort-1.30.0-amd-7800x3d.json.txt");
const PROVENANCE: &str =
    include_str!("../fixtures/search-model/linux-x64-ort-1.30.0-amd-7800x3d.provenance.json");
const SCRIPT: &str = include_str!("../fixtures/search-model/node-cpu-reference.ts.txt");
const LEGACY_SHA256: &str = "fc735637f05dfd8aebce2ea75e1f5704f37248c4404ab40ef0117ded0dfcc133";
const MAC_RUNTIME: &str = "685d2be5dba1309c89d3a5324b7fd06a5c42f1a61bfea32d14c4cc28d072121d";

pub fn qualified_fixture() -> Value {
    let context = std::env::var("LAPIS_TEST_ONNX_REFERENCE_CONTEXT").expect(
        "Explicit model reference required: use pnpm test:search:onnx with a qualified context",
    );
    assert_eq!(
        digest(LEGACY.as_bytes()),
        LEGACY_SHA256,
        "retained reference changed"
    );
    let legacy: Value = serde_json::from_str(LEGACY).unwrap();
    let (fixture, runtime_hash) = match context.as_str() {
        "darwin-arm64-ort-1.30.0" => {
            assert_platform("macos", "aarch64");
            (legacy.clone(), MAC_RUNTIME.to_owned())
        }
        "linux-x64-ort-1.30.0-amd-7800x3d" => {
            assert_platform("linux", "x86_64");
            let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
            assert_eq!(provenance["context"], context);
            let expected_cpu = provenance["runtime"]["cpu"].as_str().unwrap();
            let cpuinfo = std::fs::read_to_string("/proc/cpuinfo")
                .expect("qualified Linux CPU identity must be observable");
            let cpus = cpuinfo
                .lines()
                .filter_map(|line| line.split_once(':'))
                .filter(|(key, _)| key.trim() == "model name")
                .map(|(_, value)| value.trim())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                cpus,
                BTreeSet::from([expected_cpu]),
                "unqualified CPU: capture an independent Node reference before selecting this context"
            );
            let reference = &provenance["reference"];
            assert_eq!(reference["retainedMacFixtureSha256"], LEGACY_SHA256);
            assert_eq!(digest(LINUX.as_bytes()), reference["outputSha256"]);
            assert_eq!(digest(SCRIPT.as_bytes()), reference["scriptSha256"]);
            assert_eq!(provenance["model"]["assets"], legacy["assets"]);
            let mut fixture: Value = serde_json::from_str(LINUX).unwrap();
            for key in ["assets", "tokenHash"] {
                fixture[key] = legacy[key].clone();
            }
            assert_eq!(
                inputs(fixture.clone()),
                inputs(legacy.clone()),
                "reference corpus, tokenization, fingerprints and token-hash output must remain exact"
            );
            (
                fixture,
                provenance["runtime"]["runtimeSha256"]
                    .as_str()
                    .unwrap()
                    .into(),
            )
        }
        _ => panic!(
            "Unqualified model reference context {context:?}; qualify the exact CPU/runtime/model profile"
        ),
    };
    let runtime = std::env::var_os("LAPIS_TEST_ONNX_RUNTIME").expect("qualified runtime path");
    assert_eq!(
        file_digest(Path::new(&runtime)),
        runtime_hash,
        "unqualified ONNX runtime binary for selected reference context"
    );
    let model = std::env::var_os("LAPIS_TEST_ONNX_MODEL").expect("qualified model directory");
    for asset in legacy["assets"].as_array().unwrap() {
        let path = Path::new(&model).join(asset["file"].as_str().unwrap());
        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            asset["bytes"].as_u64().unwrap(),
            "qualified model asset size differs"
        );
        assert_eq!(
            file_digest(&path),
            asset["sha256"],
            "qualified model asset differs"
        );
    }
    fixture
}

fn assert_platform(os: &str, arch: &str) {
    assert_eq!(std::env::consts::OS, os, "unqualified model reference OS");
    assert_eq!(
        std::env::consts::ARCH,
        arch,
        "unqualified model reference architecture"
    );
}

fn inputs(mut value: Value) -> Value {
    for group in ["queries", "chunks"] {
        for row in value[group].as_array_mut().unwrap() {
            row.as_object_mut().unwrap().remove("vector");
        }
    }
    value["mixed"].as_object_mut().unwrap().remove("vectors");
    value["raw"].as_object_mut().unwrap().remove("vector");
    value
}

fn file_digest(path: &Path) -> String {
    digest(&std::fs::read(path).expect("qualified asset must be readable"))
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
