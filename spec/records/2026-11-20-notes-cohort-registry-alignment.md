# Notes cohort registry alignment

Server dependency PR #117 merged API 0.6.11, UI 0.2.8 and Workspace 0.4.10.
Its first Renovate attempt correctly reported an artifact hold because the
published App Profile, Search and Source Editor versions did not yet have an
admitted registry channel. A later source-owned canonical-slot repair made the
two-commit pull-request head green, but the check compared that head only with
its preceding bot commit. The squash-merged `main` revision
`e4dfc2e96943fcaf3c8afdc6b51f851acf4c500d` correctly failed when compared with
its real predecessor: API 0.6.11 raises its UI peer floor to 0.2.8, which is
registry metadata evolution and not an ordinary patch-only transition.

After npm exposed concrete `latest` versions, this source-owned prerequisite
completes the coherent cohort with App Profile 0.2.8, Search 0.1.15 and Source
Editor 0.1.14. A one-use receipt binds the immutable failed-main revision,
unchanged intake policy, exact manifest/workspace/lock bytes, canonical
chapters and complete changed-path set. Only the receipt-bound selected-package
metadata and graph changes are admitted; same-version first-party metadata,
workspace settings, registry-only integrities and all other intake guards stay
strict.

Pull-request source intake now reads the immutable base SHA from GitHub's event
payload. A repair commit can no longer replace the full branch transition with
its own first parent for validation. Git/Jujutsu fallback outside a GitHub pull
request still uses the captured first parent, preserving local and pushed-main
checks. The private Server root gains no npm or product release intent.
