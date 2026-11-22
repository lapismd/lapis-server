import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { verifyIntake } from "./workspace-intake.mjs";

const fixture = JSON.parse(
  gunzipSync(
    readFileSync(
      new URL(
        "../fixtures/reviewed-tasks-mcp-app-transition.json.gz",
        import.meta.url,
      ),
    ),
  ),
);
const fresh = () => structuredClone(fixture);

test("the reviewed Tasks MCP App upgrade admits the published package", () => {
  const data = fresh();
  const facts = verifyIntake(data);
  assert.equal(
    facts.manifests["package.json"].devDependencies[
      "@lapis-notes/lapis-plugin-tasks"
    ].version,
    "0.3.0",
  );
  assert.deepEqual(data, fixture);
});

test("the ordinary patch path still rejects the Tasks minor upgrade", () => {
  const data = fresh();
  data.changedFiles = [
    "package.json",
    "pnpm-lock.yaml",
    ...Object.keys(data.documents),
  ];
  assert.throws(() => verifyIntake(data), /only compatible forward patches/);
});

for (const [name, mutate] of [
  ["another parent", (data) => (data.sourceSha = "f".repeat(40))],
  ["a changed manifest", (data) => (data.files["package.json"] += "\n")],
  ["a changed lockfile", (data) => (data.files["pnpm-lock.yaml"] += "\n")],
  ["another source path", (data) => data.changedFiles.push("Cargo.toml")],
  [
    "changed canonical prose",
    (data) => (data.documents[Object.keys(data.documents)[0]] += "\n"),
  ],
]) {
  test(`the Tasks MCP App receipt rejects ${name}`, () => {
    const data = fresh();
    mutate(data);
    assert.throws(() => verifyIntake(data), /binding mismatch/);
  });
}
