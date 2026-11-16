# Repository remotes v2.3.0 publication

This fresh receipt closes the hosted presentation delivery gate for
SRV-SRV-004 and adds public-image evidence for SRV-SRV-015/016. Earlier
source, transport and browser receipts remain immutable. Live external forge
authorization and Desktop distribution retain their independent gates.

## Published source and assets

Backend PR #70 merged as `c32b5ae48b24c98dfa8de2004cf68e0496cd63bd`.
Presentation PR #93 merged as `da8f541cfe530221bb492e16b936a442f7ad04ff`.
Version PR #94 merged as `f6207f7378ff9c7749dd5bdb8481ae18832a6f7b`,
the exact source of GitHub release
[Server v2.3.0](https://github.com/lapismd/lapis-server/releases/tag/v2.3.0).
Main CI run `43789725746`, CLI release `43789767959` and versioned image
publication `43789765368` all succeed.

The public OCI index is
`sha256:256bb195389868760305909a36baad2cce1028c54717130c4a87f25f579d7d63`.
Its Linux AMD64 manifest is
`sha256:7460174dae2218aaf5a83ee215601cf4fc09219e5405fa605c4a9b66f9995cf3`;
ARM64 is
`sha256:3d02d3369390031dc3fed3f2537198b9e71dcfb6c4ab7bf7a40887f188431e6e`.
Both platform configurations identify the exact v2.3.0 source and default
`lapis` user. This receipt executes ARM64; AMD64 publication/configuration
inspection is not an AMD64 runtime claim.

All three downloaded CLI archives match the published checksum files and
`cli-manifest.json`, whose source also matches the tag:

| Target      |   Bytes | SHA-256                                                            |
| ----------- | ------: | ------------------------------------------------------------------ |
| Linux ARM64 | 6239399 | `287b88bb6e899a5f18d2610240013cae205187ea404d1a5bf57b9d109f1c68ff` |
| Linux x64   | 6375658 | `5bf329f300ebc40083b2c7e928349235fe6d7cb02c0ffd9ce87ceb8402c5c83f` |
| macOS ARM64 | 5587325 | `ea24e4d97e6f05a6b47e27c2d23e41d9b4af67109e7289d221e67efb16ccdecf` |

## Exact public-image regression and cold acceptance

The disposable browser journey runs the actual image's bundled renderer and
server, with no renderer mount, mock services, user profile or root override.
The fixture creates a Git-backed JJ original, independent generated data/key
volumes and a random 32-byte operator key. The runtime key volume is read-only;
the process's actual UID is asserted to equal 1993.

Before state: the same journey against published backend image
`sha256:8280491f4ffbc49b120417552d559bf13bbfe409f2e861e5697837e1063242d5`
reaches login, vault and settings, then fails once at the missing
`Remotes and credentials` heading. This retains the real published UI gap.

After command:

```sh
LAPIS_REMOTES_IMAGE=ghcr.io/lapismd/lapis-server@sha256:256bb195389868760305909a36baad2cce1028c54717130c4a87f25f579d7d63 \
node .local/public-image-remotes.mjs
```

One complete cold journey passes with zero failures. Both settings targets,
manual save, default-off backup synchronization, encrypted token custody,
browser detach, actual container restart, persisted connection/credential,
forgetting and guarded vault closure pass. Token absence is checked in browser
local/session storage, all vault/private files and container logs. Only the
fixture's exact generated container/volumes are removed afterward.

The ignored local driver is supplementary release evidence; the tracked
`web/e2e/rust-server-remotes.spec.ts` provides the maintained browser regression.
The public journey performs no remote transfer and does not replace the merged
real HTTPS/SSH rejection, locked-store and uncertain-outcome receipts.

## Review boundary

Normal released dependency ranges and frozen installation, 162 units, 76
interaction/accessibility cases, five static cases, builds and the full cold
history lane (three passes/two existing opt-in skips) were qualified before
merge in the [presentation receipt](2026-11-15-hosted-remotes-presentation.md).
No visual baseline, timeout, quarantine or release protection is changed.
No application-data migration is required. Live external forge/device sign-in
is still independently unqualified; this release does not claim that coverage.

The first documentation check passes validation/book but reports one missing
SRV-GOV-001 canonical mapping for the new receipt. Adding the publication link
to the owning governance chapter preserves the mapped gate; the exact
`pnpm spec:check` is rerun before committing.
