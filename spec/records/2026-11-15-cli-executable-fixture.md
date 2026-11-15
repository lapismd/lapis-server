# CLI executable fixture descriptor isolation

SRV-CLI-001 production behavior is unchanged. The complete remotes workspace
qualification reaches the pre-existing CLI installation test family and reports
8 passes and 1 failure: the alternate-source executable fails to start with
Linux ETXTBSY (`Text file busy`) at installation.rs:169. The same exact test
passes in isolation, and its next nine-test family run also passes. This is an
intermittent executable-fixture boundary, not a proven installer-policy defect.

The alternate binary was copied by a parent thread in a multithreaded test
runner while other tests spawned children. Those process creations can inherit
a writable executable descriptor until exec closes CLOEXEC descriptors. The
fixture now copies through a completed child process, keeping executable writer
descriptors out of that runner. It does not add retries, sleeps, quarantines,
serialization or relaxed assertions. Explicit source switching and all tamper,
repair and foreign-data refusals remain asserted.

After the fixture change, the exact originally failing acceptance passes once.
The entire CLI package passes 25 tests in seven suites, with zero failures or
ignores. The complete native workspace then passes 868 tests with zero failures and
10 explicit opt-in ignores across 174 suites. Strict whole-workspace all-target
Clippy (`-D warnings`) and build pass. The workspace run precedes a final
network-only test refinement; Clippy/build include that refinement. No CLI or
production source changes follow its full passing run. Native remotes source and actual host acceptance
remain separate; this fixture fix qualifies neither release.
