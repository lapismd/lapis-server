# Qualified model reference contexts

These fixtures qualify exact CPU/runtime/model contexts, not all ONNX execution
providers, CPUs or operating systems. `LAPIS_TEST_ONNX_REFERENCE_CONTEXT` must name
the context explicitly; unknown or mismatched profiles fail closed.

- `darwin-arm64-ort-1.30.0` retains the existing API fixture byte-for-byte.
  Its original qualification is recorded in
  [the encoder receipt](../../../../../spec/records/2026-10-24-rust-core-onnx.md).
- `linux-x64-ort-1.30.0-amd-7800x3d` uses the separate unchanged Node output here.
  The provenance JSON pins Node, Transformers, ORT, the library hash, CPU model,
  container image/resources, reference source/script, capture time and all five
  model assets. No Rust output supplies an expected vector.

The `.json.txt` and `.ts.txt` extensions retain the original reference output and
script bytes independently of formatting tools. The script was originally run
from `.local/search-semantic-cpu-reference.ts` in its recorded frozen Notes
source. Reproduction must preserve that location, installed versions and model
assets; it writes a new `.local/search-semantic-cpu-reference.json`. Reproducing a
reference is a separate qualification action, never part of ordinary Rust tests.

The test helper compares all corpus text, token arrays, batch dimensions,
fingerprints and token-hash output against the retained fixture. Only separately
captured CPU vectors differ. The existing `1e-5` vector and `0.0002` semantic-score
tolerances, individual/batched/raw cases and lifecycle checks remain unchanged.
