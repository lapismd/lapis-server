# Published CLI UI registry adoption

Notes source PR 116 and version PR 117 are merged. Release run 43657283043
published `@lapis-notes/ui@0.2.0`; its first verification attempt exhausted the
bounded wait while `@lapis-notes/api@0.6.1` was still processing, then attempt 2
passed registry installation, GitHub release creation and dependency notification.

The UI artifact has integrity
`sha512-vQ0iXxbzzsUuUPY692STIO5YemZP424vBghHlD7wSVu3VhM7v0SYswENZU0KmkqvmBER+WbgUWyLxqCBmCjZJA==`
and exports `./cli` from `./dist/cli/index.js`. The server now consumes the
normal `^0.2.0` registry range and lock entry. A frozen install loaded that exact
version and export; `pnpm check` passed with zero Svelte diagnostics and the App
ownership audit passed. `pnpm spec:check` passed all three policy lanes, and
`pnpm build` completed the production hosted-UI bundle.

This closes the shared-UI registry-adoption gate only. Published CLI archives,
ARM64 image acceptance and the remaining supported-platform release gates stay
pending.
