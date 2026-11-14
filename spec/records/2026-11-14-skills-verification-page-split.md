# Skill verification page size regression

Server CI run `43632219043`, job `132561143244`, failed `pnpm spec:check`
on feature source `80ffccd0`: `spec/src/verification.md` contained 33,201 UTF-8
bytes, exceeding the existing 32,768-byte limit. Its fail-fast cancellation step
cancelled the remaining native, image, configuration and UI jobs. Those job
conclusions do not establish independent product failures.

The exact local `pnpm spec:check` target reproduced the same single byte-limit
failure. SRV-OPS-005/006/007 now retain their statuses and evidence in
`spec/src/verification/plugin-skills.md`, using the existing multi-page
verification selector. Root verification is 32,679 bytes; the size limit and
one-to-one verification checks are unchanged. SUMMARY, the metadata-only
Storybook mirror and its navigation order include the dedicated page.

The first split check identified moved-link and mirror/order updates; these
were corrected. Final `pnpm spec:check` passes validation, mdBook, spec-first and
App ownership. The following formatting lane also identified the previously
unformatted hosted skill transport test; only its formatting was changed.
Its exact unit target and the full formatting lane pass.

This correction changes documentation structure and test formatting only. A new
PR head restarts CI; cancelled jobs still require qualification on that head.
