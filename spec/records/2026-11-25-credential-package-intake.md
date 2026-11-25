# Notes credential package intake

The hosted Server renderer needs the installed Notes API and App Profile built
from Notes PR #204. The registry lock keeps one API and one Workspace context
across host and profile. It resolves API 0.6.16, App Profile 0.3.2 and Workspace
0.4.11 through normal published ranges. The API and App Profile npm
checksums match the checked Notes release archives.

The reviewed receipt binds the exact parent, package manifest, registry-only
lockfile, canonical dependency slots and affected paths. The new Notes cohort
changes first-party package metadata and edges. The normal check continues to
require the same direct importer scope, patch-compatible ranges, immutable
same-version packages and registry integrity.
