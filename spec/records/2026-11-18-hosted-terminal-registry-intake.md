# Hosted Terminal registry intake

This receipt advances SRV-HST-020 from archive-only local UI qualification to
normal registry consumption. Public-image delivery remains a separate gate.

## Owning package release

`@lapis-notes/lapis-plugin-terminal@0.1.0` was published manually to npm from
Terminal source `69918c564210` and tagged/released as `v0.1.0`. The registry
tarball integrity is
`sha512-rGHZKf4ZaSOUzhl5Xmtkl3yfqie9U2ePjz3Myr6GVXOQa3aB1XRr3sQfLtpRyqye6tysE7UI/hoejYRLYh9lvA==`.
The package contains `manifest.json`, public `dist` exports and
`dist/ghostty-vt.wasm`. The temporary GitHub bootstrap token was removed after
manual publication; repository trusted-publisher registration is an npm
account-side follow-up for later versions.

## Server composition

Server consumes `^0.1.0` through the public registry and frozen pnpm lockfile.
The Vite renderer compiles the package's public Svelte source and explicitly
prebundles its `ghostty-web` dependency. The production bundle contains both
`ghostty-web-*.js` and `ghostty-vt-*.wasm`.

The hosted workspace registers required/default Content Access and
optional/default Terminal in one idempotent App-owned call. Terminal admission
requires workspace trust and optional-plugin permission. It accepts only a
relative cwd and host shell, serializes session writes, resumes bounded output
responses by cursor, handles an older missing route, and closes owned PTYs on
trust loss, disable, client retirement or final host disposal. Content Access
is not replaced or made optional.

## Source acceptance

- Terminal transport and combined registration: 2 files, 9 tests pass.
- Full Server tests pass: 109 dependency-intake cases, 8 CLI/release controls
  and 228 unit tests. Canonical specification validation, mdBook, spec-first
  mapping and App ownership pass.
- Server check: dependency intake, zero-warning Svelte/type diagnostics, App
  ownership audit and release-workflow controls pass.
- Production Vite build passes and emits Ghostty JavaScript and WASM assets.
- A cold locked Rust Server build completes; its existing linker-size warning
  remains non-fatal.
- Real hosted Terminal acceptance passes 2/2 for fresh preview and development
  renderers. Both cases create a native PTY, render Ghostty and write the proof
  file inside the admitted vault.
- Production Storybook interaction/accessibility passes 76/76, static build
  passes and built-static browser acceptance passes 5/5. No visual baseline is
  updated.
- The complete shared-host browser group passes 9/9, the real static Rust host
  passes 1/1 and native plugin operations pass 1/1. The wider command harness
  also passes 14 server cases with 2 existing opt-in skips, 3 History cases
  with 2 existing opt-in skips and 2 Properties cases. Its first Tasks run hit
  the historical retained-editor shutdown deadline after product assertions;
  the unchanged exact case then passed 1/1 and the full Tasks group passed
  15/15.

The dependency-intake extension retains an empty managed maintenance note and
an immutable one-use source receipt. Its macOS fixture now canonicalizes the
temporary root before enforcing the unchanged realpath/regular-file policy;
the exact consumption suite passes 15/15 after 5/15 pre-fix failures.

## Remaining gates

Hosted pull-request/main CI, immutable source merge, both public image
platforms and actual cold public-image Terminal acceptance remain required.
No `latest`, commit-tag or ARM64 runtime claim is made by this local receipt.
