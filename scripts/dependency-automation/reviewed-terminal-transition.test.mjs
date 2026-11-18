import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { verifyIntake } from "./workspace-intake.mjs";

const fixture = JSON.parse(
  gunzipSync(
    readFileSync(
      new URL(
        "../fixtures/reviewed-terminal-transition.json.gz",
        import.meta.url,
      ),
    ),
  ),
);
const fresh = () => structuredClone(fixture);

test("real Terminal registry addition admits exact immutable source only", () => {
  const data = fresh();
  const saved = structuredClone(data);
  const facts = verifyIntake(data);
  assert.equal(
    facts.manifests["package.json"].dependencies[
      "@lapis-notes/lapis-plugin-terminal"
    ].version,
    "0.1.0",
  );
  assert.deepEqual(data, saved);
});

for (const [name, mutate] of [
  ["wrong parent", (data) => (data.sourceSha = "f".repeat(40))],
  ["wrong base policy", (data) => (data.basePolicyBytes += " ")],
  ["wrong package", (data) => (data.files["package.json"] += "\n")],
  ["wrong lock", (data) => (data.files["pnpm-lock.yaml"] += "\n")],
  [
    "missing reviewed implementation",
    (data) =>
      (data.changedFiles = data.changedFiles.filter(
        (path) => !path.endsWith("reviewed-terminal-transition.mjs"),
      )),
  ],
  ["extra path", (data) => data.changedFiles.push("Cargo.toml")],
])
  test(`Terminal registry addition rejects ${name}`, () => {
    const data = fresh();
    mutate(data);
    assert.throws(() => verifyIntake(data));
  });
