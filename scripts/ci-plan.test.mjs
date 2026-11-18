import assert from "node:assert/strict";
import { test } from "node:test";
import { affectedPackages, impact } from "./ci-plan.mjs";

const metadata = {
  workspace_root: "/repo",
  workspace_members: ["protocol", "core", "http", "mcp", "cli", "server"],
  packages: [
    ["protocol", "lapis-app-protocol", []],
    ["core", "lapis-app-core", ["lapis-app-protocol"]],
    ["http", "lapis-content-http", ["lapis-app-core"]],
    ["mcp", "lapis-content-mcp", ["lapis-content-http"]],
    ["cli", "lapis-cli", ["lapis-app-protocol"]],
    ["server", "lapis-server", ["lapis-content-mcp"]],
  ].map(([id, name, dependencies]) => ({
    id,
    name,
    manifest_path: `/repo/crates/${name}/Cargo.toml`,
    dependencies: dependencies.map((dependency) => ({ name: dependency })),
  })),
};
const plan = (files, options = {}) =>
  impact(files, { metadata, root: "/repo", ...options });

test("Rust ownership includes reverse dependents without unrelated packages", () => {
  assert.deepEqual(
    affectedPackages(
      ["crates/lapis-content-http/src/lib.rs"],
      metadata,
      "/repo",
    ),
    ["lapis-content-http", "lapis-content-mcp", "lapis-server"],
  );
  assert.deepEqual(plan(["crates/lapis-cli/src/main.rs"]).rustPackages, [
    "lapis-cli",
  ]);
});

test("dependency automation and specification changes skip Rust", () => {
  const result = plan([
    ".changeset/lapis-dependencies.md",
    "package.json",
    "pnpm-lock.yaml",
    "scripts/dependency-automation/reviewed-tasks-transition.mjs",
    "scripts/dependency-automation/reviewed-tasks-transition.test.mjs",
    "scripts/fixtures/reviewed-tasks-transition.json.gz",
    "spec/src/current/governance.md",
  ]);
  assert.deepEqual(result.rustPackages, []);
  assert.equal(result.tooling, true);
  assert.equal(result.renderer, true);
  assert.equal(result.modLoader, false);
  assert.equal(result.imageTooling, false);
});

test("Mod and image ownership stay separate from the root Rust workspace", () => {
  const mods = plan(["crates/lapis-container-mods/src/main.rs"]);
  assert.equal(mods.modLoader, true);
  assert.equal(mods.imageTooling, true);
  assert.deepEqual(mods.rustPackages, []);
  const image = plan(["crates/lapis-server/Dockerfile"]);
  assert.equal(image.imageTooling, true);
  assert.equal(image.modLoader, true);
});

test("shared and unknown inputs fail closed", () => {
  for (const file of [
    "Cargo.lock",
    "rust-toolchain.toml",
    ".github/workflows/ci.yml",
    "scripts/ci-plan.mjs",
    "unknown.file",
  ]) {
    const result = plan([file]);
    assert.deepEqual(result.rustPackages, [
      "lapis-app-core",
      "lapis-app-protocol",
      "lapis-cli",
      "lapis-content-http",
      "lapis-content-mcp",
      "lapis-server",
    ]);
    assert.equal(result.modLoader, true);
    assert.equal(result.renderer, true);
    assert.equal(result.imageTooling, true);
  }
});

test("documentation-only changes run governance alone", () => {
  const result = plan(["README.md", "spec/src/current/governance.md"]);
  assert.deepEqual(result.rustPackages, []);
  assert.equal(result.tooling, false);
  assert.equal(result.configure, false);
  assert.equal(result.renderer, false);
  assert.equal(result.modLoader, false);
  assert.equal(result.imageTooling, false);
});
