# Affected CI and toolchain

The pre-change Server workflow had no changed-path planner. Every pull request
ran workspace-wide Clippy/tests, both OCI Mod architectures, both runtime-image
architectures and complete hosted renderer acceptance. PR104 changed dependency
automation, specifications, package metadata and the npm lock without changing
Rust source, but its Rust job still spent 11 minutes 25 seconds in the complete
workspace test command.

The new schema1 planner binds the exact pull-request base and source, maps Rust
files through Cargo metadata, adds transitive reverse dependents and fails closed
for shared or unknown inputs. Its PR104-shaped regression selects governance,
dependency tooling and hosted renderer acceptance while selecting zero root Rust
packages, no Mod qualification and no image tooling. Direct CLI source selects
only `lapis-cli`; shared Cargo/CI and unknown inputs select all six packages and
every downstream gate. A required aggregate accepts only successful selected
gates and explicit unselected skips.

The source-free `lapis-server-ci` image extends the immutable shared toolchain
and adds Rust1.99, Clippy/rustfmt, cargo-nextest0.9.152, jj0.45.1 and native build
libraries. Static checks reject repository-source, dependency and mutable-base
intake. Publication stays manual and produces native AMD64/ARM64 manifests with
provenance, SBOM and an attestation. The manifest deliberately has no digest:
runner provisioning remains in Server CI until the protected image publication
records an immutable consumer reference.

Local focused qualification initially passes nine of nine planner, aggregation
and image-source cases. The pre-existing broad Node test command reports113 of
119 passing: five workspace-consumption fixtures reject their checkout links as
non-regular inputs, and plugin operations lacks its prerequisite Rust binary.
Those failures preceded this change and are outside the affected CI boundary. A
real `lapis-app-protocol` selection passes Clippy and all44 package tests through
`scripts/ci-rust.mjs`, without compiling or testing unrelated workspace crates.
The first hosted attempt exposed one aggregate-checkout omission: the existing
independent-intake regression failed in both Node consumers because Required CI
did not request complete history. Adding `fetch-depth:0` to that exact checkout
passes the focused regression; no test assertion or intake boundary changed.
Hosted affected/full execution, image publication, digest adoption and matched
Nextest timing remain open; no performance or release qualification is claimed.
