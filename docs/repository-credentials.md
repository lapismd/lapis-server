# Server repository credential custody

The native `credentials::CredentialStore` adapter is source infrastructure.
Application startup, authenticated routes and Repository History settings have
not adopted it yet. Do not infer hosted availability from its native tests.

The caller supplies a stable installation UUID, an installation-private 0700
directory outside every admitted vault, and an absolute operator key-file path.
The key file must contain exactly 32 random bytes. It must be mounted separately
from the profile and encrypted-record backups; it cannot live in the credential
directory. Missing keys, wrong sizes, wrong keys and verifier failures keep the
store locked. There is no plaintext or browser-storage fallback.

For eventual deployment, provision the key through the operator's secret manager
or a protected file and mount it read-only outside `/data`. Persist the encrypted
store and installation identity across container replacement. Keep the key in a
separate recovery system; an encrypted profile backup must never include its
key. Restore the same installation identity and key together with its ciphertext.
Losing the key requires forgetting the unusable credentials and signing in again;
changing the key bytes is not a supported rotation procedure. Never overwrite
ciphertext to recover from a locked store.

Records use AES-256-GCM with a fresh random 96-bit nonce. Authenticated data binds
the format, installation, authenticated owner and credential UUID. Each token is
scoped to its approved HTTPS repository endpoint or explicit origin/account
grant; a repository grant never expands to sibling paths. Native inventory contains
metadata only; resolving a stored secret returns a native-only value without
serialization or formatting implementations. Save, replace and forget use the
private native writer and persist across restart.

The host must resolve the authenticated owner before calling this adapter and
must never accept filesystem paths or installation identity from HTTP requests.
A different owner, unapproved origin or record identity cannot reuse a credential.
Repository connection configuration stores opaque references separately from
custody. Neither credential records nor the key belong in vault checkpoints,
plugin data, exports, browser storage or logs.

Host-managed SSH remains a separate transport: configure the agent/default
identity and known hosts on the executing server. This adapter accepts tokens,
not uploaded or generated SSH private keys. Preserve host-key verification.
