# Sync browser consent — 15 November 2026

This slice extends account HTTP source `4739af2c` in the independent Server task
repository. It connects the compiled server's account request to its static
hosted browser UI. Desktop intake and OS custody remain separate open work.

## Implemented behavior

Account initiation returns an approval URL made only from the configured public
origin and fresh UUID. The ordinary hosted entry point selects the consent
screen when `sync-connect` is present. Malformed links cause no HTTP requests.
The screen uses the retained Rust cookie client for owner sign-in, request
inspection and CSRF-bound decisions. It never redeems the native verifier or
receives the discovery bearer.

The owner reviews the requesting device's name, UUID and discovery-only scope.
No mutation occurs on inspection or sign-in. Approve and Reject are explicit
keyboard-operable actions. Expired, rejected and consumed requests have no
decision buttons. A failed decision response retires those buttons and asks for
reload/inspection instead of replay. Teardown aborts inspection and fences late
responses. Password state is cleared after sign-in attempts.

## Before and after

The previous source had no browser account consent screen or browser acceptance
journey. The first component suite passed 5/5. Type checking exposed four
diagnostics caused by naming the view state `state`, which collided with Svelte
rune typing; renaming it to `viewState` restored **0 errors, 0 warnings**.

The first desktop/narrow story run passed 4/6 and failed both Expired cases:
the role query found the loading status before inspection completed. Waiting
for the required expired text retained the assertion and repaired the readiness
boundary. The same focused story target then passed **6/6**, followed by the
complete **64/64** interaction/accessibility lane. No shared helpers or baselines
were weakened.

## Verification

- Final component suite: **6/6**, including approve/reject, sign-in continuity,
  malformed links, unknown outcomes and unmount-before-response fencing.
- Complete Web unit suite: **138/138** in 32 files; CLI distribution **4/4**.
- Exact new real-server browser journey: **1/1**. A task-owned compiled Rust
  process and freshly built static renderer use an isolated profile. The request
  survives a cold server restart; owner sign-in and keyboard approval issue
  only discovery authority; ordinary vault access is refused; browser storage
  contains neither verifier nor bearer. A further server restart preserves the
  credential, then sign-out revokes discovery. Reload shows consumed consent
  without another approval action.
- Complete affected hosted browser lane: **6/6**, including existing static,
  preview and development hosted/content-API journeys.
- Focused desktop/narrow Storybook: **6/6**; complete Storybook **64/64**,
  with axe failures enabled. Fresh static Storybook build: passed.
- Exact static narrow consent case: **1/1**; complete static catalogue **5/5**.
- Existing account HTTP target after the server-generated approval URL: **2/2**.
- Strict workspace/all-target Clippy, type/Svelte, App ownership, production Web
  build, formatting and canonical specification lanes: passed.
- QMD search resolves SRV-SYNC-014 under installed Node 24.10.0.

Built `web/dist/index.html` SHA-256:
`c85238f44a4a81fe115098547f67f8bbf7e5aec2e104ef8c5fa8ce20197f1907`.
This identifies the tested entry artifact, not a published or installed bundle.
The pre-existing Web chunk-size and macOS linker notices remain visible.

Visual review remains pending. This does not qualify native Desktop account
custody, endpoint/server enrollment, encrypted transfer, continuous participation
or independent-device convergence.
