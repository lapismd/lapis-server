# Verification

| ID          | Status      | Evidence                                                                                                                                                         |
| ----------- | ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SRV-GOV-001 | Implemented | Spec-validator checks file limits and one verification row per requirement. CI installs mdbook 0.5.4 before that check. Notes CI does not run server acceptance. |
| SRV-SRV-001 | Implemented | Dockerfile exposes 8787, volume `/data`, and `readyz`. Cargo.toml pins lapis-ai tag v0.2.0.                                                                      |
| SRV-SRV-002 | Implemented | jj-store crates remain path dependencies. The manifest check rejects pull request 22's head as a git revision.                                                   |
