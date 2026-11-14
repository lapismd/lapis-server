import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtemp,
  mkdir,
  writeFile,
  readFile,
  rm,
  utimes,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
const archiveScript = resolve("scripts/cli-archive.mjs");
const publishScript = resolve("scripts/publish-cli.mjs");
async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), "lapis-cli-distribution-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  await writeFile(join(root, "package.json"), '{"version":"2.0.3"}');
  await mkdir(join(root, "crates/lapis-cli"), { recursive: true });
  await writeFile(join(root, "crates/lapis-cli/README.md"), "CLI fixture");
  for (const target of [
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
  ]) {
    const path = join(root, ".local/rust-core-target", target, "release");
    await mkdir(path, { recursive: true });
    await writeFile(join(path, "lapis"), "fixture executable", { mode: 0o755 });
    const built = spawnSync(process.execPath, [archiveScript, target], {
      cwd: root,
      env: { ...process.env, GITHUB_SHA: "a".repeat(40) },
      encoding: "utf8",
    });
    assert.equal(built.status, 0, built.stderr);
  }
  return root;
}
test("CLI archives contain the executable and bind exact bytes to source and target", async (t) => {
  const root = await fixture(t);
  const directory = join(root, ".local/cli-release");
  const manifest = JSON.parse(
    await readFile(join(directory, "x86_64-unknown-linux-gnu.json")),
  );
  const bytes = await readFile(join(directory, manifest.file));
  assert.equal(manifest.sourceCommit, "a".repeat(40));
  assert.equal(
    manifest.sha256,
    createHash("sha256").update(bytes).digest("hex"),
  );
  assert.equal(manifest.size, bytes.length);
  assert.deepEqual(
    execFileSync("tar", ["-tzf", join(directory, manifest.file)], {
      encoding: "utf8",
    })
      .trim()
      .split("\n"),
    ["lapis", "README.md"],
  );
});
test("CLI publication rejects changed bytes before attempting a release upload", async (t) => {
  const root = await fixture(t);
  await writeFile(
    join(root, ".local/cli-release/lapis-v2.0.3-linux-x64.tar.gz"),
    "tampered",
  );
  const result = spawnSync(process.execPath, [publishScript], {
    cwd: root,
    env: {
      ...process.env,
      GITHUB_SHA: "a".repeat(40),
      GITHUB_REF_NAME: "v2.0.3",
    },
    encoding: "utf8",
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /CLI checksum mismatch/);
});
test("CLI publication rejects archives from another source revision", async (t) => {
  const root = await fixture(t);
  const result = spawnSync(process.execPath, [publishScript], {
    cwd: root,
    env: {
      ...process.env,
      GITHUB_SHA: "b".repeat(40),
      GITHUB_REF_NAME: "v2.0.3",
    },
    encoding: "utf8",
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /CLI provenance mismatch/);
});

test("CLI archive bytes are reproducible when filesystem timestamps change", async (t) => {
  const root = await fixture(t);
  const archive = join(
    root,
    ".local/cli-release/lapis-v2.0.3-linux-x64.tar.gz",
  );
  const before = await readFile(archive);
  await utimes(
    join(
      root,
      ".local/rust-core-target/x86_64-unknown-linux-gnu/release/lapis",
    ),
    100,
    100,
  );
  const rebuilt = spawnSync(
    process.execPath,
    [archiveScript, "x86_64-unknown-linux-gnu"],
    {
      cwd: root,
      env: { ...process.env, GITHUB_SHA: "a".repeat(40) },
      encoding: "utf8",
    },
  );
  assert.equal(rebuilt.status, 0, rebuilt.stderr);
  assert.deepEqual(await readFile(archive), before);
});
