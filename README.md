# Lapis server

The public image is `ghcr.io/lapismd/lapis-server`. It listens on port `8787`,
stores the profile on `/data`, and reports readiness at
`http://127.0.0.1:8787/readyz`.

The Rust workspace is `lapis-server`, `lapis-app-core`, and
`lapis-app-protocol`. History and vault crates are path dependencies on a
sibling `jj-store` checkout. After
[jj-store #22](https://github.com/stevejuma/jj-store/pull/22) merges, replace
those paths with a Cargo git dependency on
`https://github.com/stevejuma/jj-store` at the merged revision. Do not use the
unmerged head `35356e655765143b29a6c9b1518ef3b91d04837d`. There is no crate tag.

`lapis-ai-service` is git tag `v0.2.0` at version `0.2.0`.

```sh
pnpm install
pnpm spec:check
pnpm configure
```

`pnpm configure -- --metadata` asks Cargo to read the package graph. That
resolves the local jj-store path. It is not a compile and not a release.
