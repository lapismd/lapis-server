# CLI verification

| ID          | Status  | Evidence                                                                                                                        |
| ----------- | ------- | ------------------------------------------------------------------------------------------------------------------------------- |
| SRV-CLI-001 | Partial | `crates/lapis-cli/tests/installation.rs`: eight installation regressions; desktop and packaged acceptance pending.               |
| SRV-CLI-002 | Partial | Hosted settings and bounded release metadata adapter; published-download acceptance pending.                                    |
| SRV-CLI-003 | Partial | Docker builds/copies `lapis`; `scripts/test-image-slim.sh` checks runtime PATH execution; final image acceptance pending.       |
| SRV-CLI-004 | Partial | Four archive/release regressions and native Linux archive install/remove acceptance; hosted multi-platform publication pending. |
