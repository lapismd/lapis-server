# Authentication verification

| ID          | Status      | Evidence                                                                                                                                                                                                      |
| ----------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SRV-SRV-017 | Implemented | Hosted Rust tests exercise HTTP and MCP restart, scope, one-time reveal and durable revocation. Browser tests exercise creation and revocation in static, preview and development modes.                      |
| SRV-SRV-018 | Partial     | Hosted Rust tests cover discovery, owner consent, PKCE, MCP negotiation, restart, refresh rotation and replay refusal. Live Codex and Claude Code connected over HTTPS. ChatGPT web qualification is pending. |
| SRV-HST-023 | Implemented | Hosted Chromium tests use a virtual authenticator to enroll a passkey, sign in after reload and remove it.                                                                                                    |
| SRV-HST-024 | Implemented | Hosted Rust tests and the generated contract cover HTTP operations. A hosted Chromium test executes an authenticated request in Swagger UI.                                                                   |
