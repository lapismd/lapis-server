# CLI release target preparation

The release workflow explicitly requests its matrix target from the pinned Rust
toolchain action. This keeps macOS ARM64 build-only preparation valid even when
runner host architecture differs. The pinned action declares the `targets`
input and installs it through rustup. This changes workflow preparation only;
no native CLI implementation or tested archive contents changed.

Validation: workflow YAML formatting and specification checks pass. The four
existing distribution regressions pass. Actual hosted multi-platform execution
and publication remain release gates; no release was dispatched.
