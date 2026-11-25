# Installation credential custody

SRV-SRV-015 and SRV-SRV-016 share one authenticated Server credential store.
Repository and forge use reserved native domains. Plugin records bind the
installation, owner, plugin ID and entry name, and the admitted client owns
revocation. The hosted attachment only advertises credential discovery when
its installation has plugin grants.

- Strict Clippy and the full native Server/Core suite passed with 887 tests
  passing and 11 existing skips. The 19 credential cases and four repository HTTP cases also passed after integrating the
  background runtime. One SSH transport case remains opt-in.
- A cold compiled Linux Server retained HTTPS Repository custody when switching
  from the Base64 environment setting to a raw key file with the same key. The
  serving process's environment excluded the startup key. Separate tests cover
  missing, malformed and wrong keys, record tampering and cross-scope rejection.
- The credential lease HTTP test covers two plugin scopes, cross-client refusal,
  explicit revocation and cold encrypted persistence.
- The renderer passed zero-warning Svelte checks, 243 unit tests, a production
  build, 110 catalog interactions and 10 static catalog cases using the public
  build archives from Notes PR #204. The cold startup budget passed with the
  default empty credential approval list. Registry package intake and exact-head
  hosted qualification remain separate.
- In the hosted suite, the Tasks lost-response case timed out while looking for
  the sign-in screen during teardown after a successful recovery. The exact
  case passed on a cold rerun with the existing assertion and timeout. The full
  Tasks rerun passed all 15 cases. The remaining static, PWA and MCP Apps
  hosted lanes passed six cases.

The compiled browser acceptance exercise activates an approved installed ESM
plugin through the production App Profile. The plugin creates its own entry
with `Plugin.secrets` and reopens it after a cold Server restart with the same
key delivered as a raw key file. The test uses the checked Notes API 0.6.16
and App Profile 0.3.2 release candidate archives.
