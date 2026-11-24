# Repository remotes on the server

The server profile provides native repository settings for its single installation
owner. Browser sessions and the installation bearer token authenticate that same
owner. Client attachments still belong to their authenticated principal and vault
generation; another session cannot reuse an attachment. A file-only server without
`LAPIS_SERVER_DATA_ROOT` does not enable remote management.

## Provision token storage

Configure one Server credential key using [the hosted deployment guide](hosted-server.md#credential-key).
Optional `LAPIS_SERVER_GITHUB_CLIENT_ID` supplies the
operator's GitHub OAuth application client ID for device sign-in. Without it,
raw HTTPS tokens and GitHub/Forgejo token accounts remain available.

Private `/data/repository-remotes/` contains a stable installation/owner identity,
separate account metadata and per-vault connection/recovery state, and an
AES-256-GCM credential store. Preserve this directory across container replacement.
The key is separate from the ciphertext and must be backed up through a separate
operator secret-management channel. Restoring encrypted backups requires the
original key and identity file. A missing, malformed or incorrect key leaves
custody visibly locked and does not overwrite existing encrypted records or fall
back to plaintext. Restore the correct mount and restart the server to unlock it.
Lost keys cannot decrypt existing credentials; reauthentication requires a
separately planned operator recovery rather than silently replacing the store.

## Host-managed SSH

The runtime image includes `openssh-client`. SSH runs on the server, using its
agent or default identities, never the browser's keys. Mount only the identities
or agent socket intended for this server. Optional
`LAPIS_SERVER_SSH_KNOWN_HOSTS` selects an operator-managed trust file outside
vault content; otherwise normal OpenSSH host trust applies. Provision verified
host keys before testing. Verification is never bypassed, and the app offers no
private-key upload or generation. Locked token custody does not disable SSH.

## Routes and operations

Authenticated routes under `/api/v3/clients/{client}/repository-remotes` accept
GET inventory and bounded POST commands. Both require the attached `vault` and
`generation` query parameters. Working-copy details use the matching
`repository-working-copy` GET route. Browser commands retain protocol, exact
origin, owner-cookie and CSRF checks; responses use `Cache-Control: no-store`.
No request selects a native path, encryption key, account directory or owner.

Saving a connection performs no network transfer. Testing checks authentication
and reachability without publishing. Fetch updates repository metadata; publishing
requires a native review of one target, branch, bookmark and commit. Original
working files and selected history pins remain untouched. New backup connections
remain manual. Automatic remote synchronization and full browser/Desktop
qualification remain pending; installing these native routes alone does not
qualify the overall feature.

## Repeatable HTTPS acceptance

In a disposable Linux validation container, build the Server with
`cargo build --locked -p lapis-server`, then run
`LAPIS_SERVER_DISPOSABLE_HTTPS=1 node scripts/test-repository-remotes-https.mjs`
as its root test user. This test installs a short-lived CA only inside that
container and removes it afterward. Never run it on an operator machine. The
script starts the compiled binary with fresh private owner/vault/credential
state; it checks real authenticated Git HTTPS, anonymous reads, exact publication,
restart, detach and locked encryption storage. It uses no real account or vault.
