import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { nextestPins, verify } from "./ci-image-tools.mjs";

const manifest = JSON.parse(await readFile(".ci/server-ci.json", "utf8"));

test("Server CI image pins its complete source-free toolchain", async () => {
  const dockerfile = await readFile(".ci/server-ci.Dockerfile", "utf8");
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.image, "ghcr.io/lapismd/lapis-server-ci");
  assert.equal(manifest.digest, null);
  assert.match(manifest.tag, /^2026\.\d+\.\d+$/u);
  assert.ok(dockerfile.includes(manifest.baseImage));
  assert.ok(dockerfile.includes(manifest.rustImage));
  assert.match(dockerfile, /rustup component add clippy rustfmt/u);
  assert.doesNotMatch(dockerfile, /COPY \. /u);
  assert.doesNotMatch(dockerfile, /(?:node_modules|Cargo\.lock|pnpm-lock)/u);
  for (const digest of Object.values(nextestPins))
    assert.match(digest, /^[a-f0-9]{64}$/u);
  assert.throws(() => verify(Buffer.from("wrong"), "0".repeat(64), "test"));
});

test("image publication remains manual and multi-architecture", async () => {
  const workflow = await readFile(
    ".github/workflows/publish-ci-image.yml",
    "utf8",
  );
  assert.match(workflow, /workflow_dispatch:/u);
  assert.match(workflow, /PUBLISH_SERVER_CI_IMAGE/u);
  assert.match(workflow, /platforms: linux\/amd64,linux\/arm64/u);
  assert.match(workflow, /provenance: mode=max/u);
  assert.match(workflow, /sbom: true/u);
  assert.match(workflow, /actions\/attest-build-provenance@v4/u);
});
