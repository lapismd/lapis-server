import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
test("the CLI binary version matches the Changesets release version", () => {
  const pkg = JSON.parse(readFileSync("package.json"));
  const cargo = readFileSync("crates/lapis-cli/Cargo.toml", "utf8");
  assert.equal(cargo.match(/^version = "([^"]+)"/m)?.[1], pkg.version);
});

import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { syncCliVersion } from "./sync-cli-version.mjs";
test("version preparation synchronizes both identities and preflights a malformed lock", (t) => {
  const root = mkdtempSync(join(tmpdir(), "lapis-cli-version-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "crates/lapis-cli"), { recursive: true });
  writeFileSync(
    join(root, "package.json"),
    JSON.stringify({ version: "2.9.2" }),
  );
  const cargo = '[package]\nname = "lapis-cli"\nversion = "0.1.0"\n';
  const file = join(root, "crates/lapis-cli/Cargo.toml");
  writeFileSync(file, cargo);
  writeFileSync(join(root, "Cargo.lock"), "invalid");
  assert.throws(() => syncCliVersion(root));
  assert.equal(readFileSync(file, "utf8"), cargo);
  writeFileSync(
    join(root, "Cargo.lock"),
    '[[package]]\nname = "lapis-cli"\nversion = "0.1.0"\n',
  );
  assert.throws(() => syncCliVersion(root, true));
  syncCliVersion(root);
  syncCliVersion(root, true);
  assert.match(readFileSync(file, "utf8"), /version = "2.9.2"/);
  assert.match(
    readFileSync(join(root, "Cargo.lock"), "utf8"),
    /version = "2.9.2"/,
  );
});
