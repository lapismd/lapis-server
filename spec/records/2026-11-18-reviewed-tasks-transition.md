# Source-reviewed Tasks registry transition

The source parent is 3c8e48a2f0f4c345edc836493419184914d3bc27. Tasks is the only
changed registry identity:0.1.1 to0.2.1, range^0.2.1. The ordinary intake
reproduction rejects one attempted minor with `only compatible forward patches
are supported` before implementation. The exact-source receipt is executable
source, not candidate-supplied approval.

Published root declarations are byte-identical and runtime ID remains `tasks`.
Version0.2.0 adds operations.json and manifest operation contributions;0.2.1
admits API0.6 and Core^0.13.2, matching ServerAPI0.6.5/Core0.13.4. Prior peers
API^0.2.3/Core^0.7.4 were stale. Both versions are Node/ESM and Deno-free.

Qualification results are recorded in the implementation handoff. Full
production acceptance is not asserted: production HIGH99, exact-main CI/image,
Pages/release/callbacks and Root activation remain separate gates. No native pin,
private product version, central policy or audit exception changes.
