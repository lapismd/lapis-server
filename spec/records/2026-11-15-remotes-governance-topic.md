# Repository remotes governance topic

Unchanged-contract confirmation for SRV-GOV-001: move the existing remotes
qualification paragraphs from `current/governance.md` into the linked
`current/governance/remotes.md` topic. Every acceptance rule is retained;
specification limits, native authority, test selection and release gates stay
unchanged. The Summary and original chapter link to the extracted topic.

Hosted Server CI run 43704884680 checked merge tree
`44364fcb8e163749170af19edaaf7f67a7a65087` for feature head
`5fc44f41f8e7fb5168415c4c60a2b87909c71183` against main
`0167f020968fcad13db3df43681b8a586df991f6`. Its specification step failed once:
the combined governance chapter reached 423 physical lines against the 400-line
limit. The workflow's existing fail-fast step then cancelled the whole run;
those cancelled jobs are not passing qualification. Local checks had passed the
shorter unmerged feature chapter. The original-source run 43700461105 also
remains cancelled; this record does not retroactively qualify it.

The extraction keeps concurrent mainline additions and remotes acceptance rules
within topic boundaries. Requalify the next pushed combined tree through the
unchanged hosted gate. No functional code, visual baseline, workflow limit or
CI cancellation policy changes in this documentation slice. The first local
check of the extraction found two mirror-wiring errors. Adding the metadata-only
Storybook mirror and its exact Summary-order entry addresses those checker
requirements without changing a product story or runtime decorator.

After those corrections, Node 24 `pnpm spec:check` passes all three specification
lanes and the App-ownership audit. Prettier passes the extracted topic, Summary,
mirror, order entry, verification row and this record. Functional suites are
outside this documentation-only slice; the next feature head retains its own
native and hosted qualification obligations.
