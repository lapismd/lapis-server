---
"@lapismd/lapis-server": minor
---

Use one encrypted Server credential store for Repository, forge accounts, and approved plugins. Supply the installation key via `LAPIS_SERVER_CREDENTIAL_KEY` or `LAPIS_SERVER_CREDENTIAL_KEY_FILE`, which replace the repository-only setting. Operators can approve trusted plugin IDs with `LAPIS_SERVER_CREDENTIAL_PLUGINS`.
