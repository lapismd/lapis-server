# Dependency toolchain

SRV-GOV-001/003 retain Node24, pnpm10.34.5 and Changesets3.0.3 for the actual
private Server version recipe. Only the validator override is reviewed as data;
all other workspace bytes and executable/configuration/formatter bindings remain
exact. The complete source-owned version command is requalified with Deno absent.
An empty actual release plan does not mint a version-PR App token, while existing
GitHub/tag/image/CLI authority and gates remain distinct. No npm package is
published by this private root, and this intake changes no product version.

One exact managed dependency Changeset may accompany a development patch only
when its YAML release map is empty. Nonempty, malformed, multiple or arbitrary
notes remain rejected; no product version intent is added.

Git and Jujutsu changed paths MUST use the same captured immutable parent as
prior input facts. Clean hosted Git checkouts MUST retain tracked managed-note
and unexpected-file scrutiny; one managed note stays bounded to8192 UTF-8 bytes
with an empty release map. This changes no source input, version or audit policy.

<!-- dependency-intake:validator-development:start -->

```json
{
  "schemaVersion": 1,
  "kind": "development-dependency-intake",
  "dependency": "@lapismd/spec-validator",
  "version": "0.2.1",
  "peer": null,
  "integrity": "sha512-aHLByppZkay1axSm8KvCwOJz0si8aDyZBXDGWW+d2VQw+66f1KLimw2rEMjYIpLdoSG9+yN5Kd1MKlAJw7xUVw==",
  "importers": {
    ".": {
      "specifier": "0.2.1",
      "version": "0.2.1"
    }
  },
  "policySha256": "9755fa086116d830df077ab716cf3821545e7b31cc28b07607b1629ca822d68e"
}
```

<!-- dependency-intake:validator-development:end -->
