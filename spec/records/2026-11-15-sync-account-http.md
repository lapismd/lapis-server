# Sync account HTTP handoff — 15 November 2026

Source boundary: Server account-consent stack at `f2a8b05d`, followed by this
HTTP slice in the independent `/tmp/lapis-sync-implementation.Vk9Fyp/lapis-server`
repository. This receipt qualifies source and real loopback HTTP only.

## Behavior

Native clients initiate ten-minute S256 requests and redeem an approved request
once. Initiation and redemption enforce protocol version 3, a 4 KiB body limit,
ten-second body deadline and origin checks; ambient cookies and Authorization
headers are refused. At most 128 unexpired requests are retained. Expired
requests are removed before new admission; capacity refuses without eviction of
live requests.

The owner inspects display metadata through an authenticated cookie route.
Inspection never returns a challenge or verifier. Approval/rejection requires
the existing owner-cookie, origin and CSRF boundary; an installation bearer
cannot make the decision. Redemption returns a discovery-only bearer whose hash
is retained by the control store.

Discovery returns only active managed-vault names and IDs, bounds its database
query to 4,097 rows and refuses a catalogue above 4,096. Configured external roots
and archived managed vaults are excluded. Sign-out revokes the presented
credential. The discovery bearer cannot attach a writer or access the ordinary
vault API. Browser presentation, native custody, endpoint enrollment and content
authority remain open.

## Verification

- Account-store tests: **4 passed, 0 failed**; includes bounded capacity and
  admission after expiry, plus retained S256/expiry/replay/reopen cases.
- Real loopback HTTP tests: **2 passed, 0 failed**. Covers pending redemption,
  missing authentication, CSRF refusal, installation-token refusal, safe owner
  inspection, approved redemption, replay, ordinary-vault denial, retained owner
  writer access, managed-only discovery, sign-out, foreign origins, ambient
  cookies and oversized requests.
- Full `cargo +1.99.0 test -p lapis-server --locked`: **154 passed, 0 failed,
  0 ignored**, across 31 test result groups including empty doc-test groups.
- First widened run encountered a connection reset in the existing
  `http_files::database::database_json_has_its_own_bound_without_raising_the_file_command_limit`
  case: **53 passed, 1 failed** in that target. The exact unchanged case passed
  **1/1** on rerun and the full package rerun passed including **54/54** in
  `http_files`. No assertion or test infrastructure was weakened. The transient
  remains recorded; no general claim of eliminating that failure is made.
- Strict workspace/all-target Clippy with warnings denied: passed.
- Specification validation, mdBook, spec-first and App-ownership: passed.
- Type/Svelte: **0 errors, 0 warnings**. CLI distribution: **4/4**; Web unit:
  **132/132**; production Web build: passed.
- Repository formatting: passed. Each new production file is within 300 lines,
  each test file within 500. Latest spec-validator is `0.2.0`, already selected.
- Doctor and QMD search pass under installed Node 24.10.0. Node 26's QMD ABI
  mismatch remains a process-selection issue; no system installation was changed.

macOS linker runs retain the existing large unwind-table warning. The Web build
retains existing large-chunk notices. These checks do not qualify an image,
Desktop window, OS key store, browser consent screen, encrypted content transfer
or independent-device convergence. Visual review remains pending.
