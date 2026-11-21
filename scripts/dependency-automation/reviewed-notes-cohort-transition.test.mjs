import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import YAML from "yaml";
import { verifyIntake } from "./workspace-intake.mjs";

const fixture = JSON.parse(
  gunzipSync(
    readFileSync(
      new URL(
        "../fixtures/reviewed-notes-cohort-transition.json.gz",
        import.meta.url,
      ),
    ),
  ),
);
const fresh = () => structuredClone(fixture);

test("real Notes cohort transition admits exact immutable source only", () => {
  const data = fresh();
  const saved = structuredClone(data);
  const facts = verifyIntake(data);
  assert.equal(
    facts.manifests["package.json"].dependencies["@lapis-notes/app-profile"]
      .version,
    "0.2.8",
  );
  assert.equal(
    facts.manifests["package.json"].dependencies["@lapis-notes/search"].version,
    "0.1.15",
  );
  assert.equal(
    facts.manifests["package.json"].dependencies["@lapis-notes/source-editor"]
      .version,
    "0.1.14",
  );
  assert.deepEqual(data, saved);
});

test("ordinary canonical patch cannot change Notes cohort metadata", () => {
  const data = fresh();
  data.changedFiles = data.changedFiles.filter(
    (path) => !path.endsWith("reviewed-notes-cohort-transition.mjs"),
  );
  assert.throws(() => verifyIntake(data));
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
        (path) => !path.endsWith("workspace-catalog.mjs"),
      )),
  ],
  ["extra path", (data) => data.changedFiles.push("Cargo.toml")],
  [
    "extra selected metadata",
    (data) => {
      const lock = YAML.parse(data.files["pnpm-lock.yaml"]);
      lock.packages["@lapis-notes/app-profile@0.2.8"].deprecated = "unreviewed";
      data.files["pnpm-lock.yaml"] = YAML.stringify(lock);
    },
  ],
  [
    "unrelated immutable metadata",
    (data) => {
      const lock = YAML.parse(data.files["pnpm-lock.yaml"]);
      lock.packages["@lapis-notes/history@0.5.1"].deprecated = "unreviewed";
      data.files["pnpm-lock.yaml"] = YAML.stringify(lock);
    },
  ],
  [
    "workspace setting",
    (data) =>
      (data.files["pnpm-workspace.yaml"] +=
        "\nonlyBuiltDependencies: [unreviewed]\n"),
  ],
  [
    "unreviewed canonical prose",
    (data) => (data.documents[Object.keys(data.documents)[0]] += "unreviewed"),
  ],
  [
    "stale canonical baseline",
    (data) =>
      (data.baseDocuments[Object.keys(data.baseDocuments)[0]] += "unreviewed"),
  ],
])
  test(`Notes cohort transition rejects ${name}`, () => {
    const data = fresh();
    mutate(data);
    assert.throws(() => verifyIntake(data));
  });
