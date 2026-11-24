# Native plugin background runtime source qualification

This receipt covers the Server-owned SRV-JOB-001 through SRV-JOB-008 source
contracts. Notes public controls, Desktop admission and actual installed-plugin
host acceptance are separate consumer gates. All execution ran on the explicitly
selected Omarchy Docker context with task-owned profiles and caches.

## Passed source boundaries

- `cargo test --locked -p lapis-plugin-jobs`: eight execution and five scheduling
  tests pass. Import denial, bounded memory, deadlines, cancellation, asynchronous
  handlers, IANA timezones, missed slots and daylight-saving boundaries are covered.
- `cargo test --locked -p lapis-app-core --test service_background_jobs`: ten
  tests pass, including zero-client work, private checkpoint recovery, dirty-editor
  refusal, uncertain outcomes, notifications, trust, durable handoff and monotonic
  schedule high-water. The final notification assertion also checks the persistent
  history contains exactly one record.
- Full `cargo test --workspace --locked` and strict all-target workspace Clippy
  pass in source qualification run d. The final strengthened Core test and strict
  Clippy pass again in run f. Run d source:
  `fc0e315f6084d21b98b0052a601a9725e8f99c2b`.
- Generated protocol bindings and schemas, protocol TypeScript and OpenAPI match
  the native exporters. OpenAPI initially failed because a CLI formatter expanded
  its JSON; canonical generation now uses the exporter script's exact formatting.
- The real bundled Tasks headless module (330,234 bytes, no imports) passes
  `packaged_handler_obeys_the_headless_engine_and_expected_broker_contract`
  in the default 16 MiB QuickJS runtime. SHA-256:
  `fa4157da9fbabe9c55fd7d34860bb6de4da085fa9c236441fe54a370429b0c27`.
  Its first fixture omitted the recurrence start date and expected one task rather
  than two. The valid recurrence fixture and its strengthened owner unit test now
  pass. No engine or retained Tasks recurrence semantics were weakened.

## Wider integration evidence and remaining gates

The in-progress hosted adapter cohort passes all 244 browser unit tests, three
focused transport tests, source specification, Svelte/type checks and production
build. The first build exhausted Node's default heap; the bounded 6 GiB heap in
its 12 GiB container passes. That cohort's new browser acceptance exposed a test
fixture broker-call loop and then a literal assertion against valid quoted YAML
keys. It now checks parsed property values and retained body content; its exact
cold browser retry and wider hosted/catalog gates remain pending at this receipt.

The runtime source slice is committed independently so Desktop can consume an
exact reviewed Git revision while consumer integration continues. This is not
an installed-plugin, native Desktop, image deployment or package-publication claim.
Visual baselines and existing unrelated source remain untouched.
