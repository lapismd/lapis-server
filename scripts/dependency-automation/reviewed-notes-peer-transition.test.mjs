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
        "../fixtures/reviewed-notes-peer-transition.json.gz",
        import.meta.url,
      ),
    ),
  ),
);
const fresh = () => structuredClone(fixture);

test("real Notes API/UI peer transition admits exact immutable source only", () => {
  const data = fresh();
  const saved = structuredClone(data);
  const facts = verifyIntake(data);
  assert.equal(
    facts.manifests["package.json"].dependencies["@lapis-notes/api"].version,
    "0.6.10",
  );
  assert.equal(
    facts.manifests["package.json"].dependencies["@lapis-notes/ui"].version,
    "0.2.6",
  );
  assert.deepEqual(data, saved);
});

test("ordinary canonical patch cannot change Notes peer metadata", () => {
  const data = fresh();
  data.changedFiles = data.changedFiles.filter(
    (path) => !path.endsWith("reviewed-notes-peer-transition.mjs"),
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
    "extra API metadata",
    (data) => {
      const lock = YAML.parse(data.files["pnpm-lock.yaml"]);
      lock.packages["@lapis-notes/api@0.6.10"].peerDependencies.i18next =
        "^999.0.0";
      data.files["pnpm-lock.yaml"] = YAML.stringify(lock);
    },
  ],
  [
    "unrelated immutable metadata",
    (data) => {
      const lock = YAML.parse(data.files["pnpm-lock.yaml"]);
      lock.packages["@lapis-notes/app-profile@0.2.2"].deprecated = "unreviewed";
      data.files["pnpm-lock.yaml"] = YAML.stringify(lock);
    },
  ],
  [
    "extra first-party removal",
    (data) => {
      const lock = YAML.parse(data.files["pnpm-lock.yaml"]);
      delete lock.packages["@lapis-notes/app-profile@0.2.2"];
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
  test(`Notes peer transition rejects ${name}`, () => {
    const data = fresh();
    mutate(data);
    assert.throws(() => verifyIntake(data));
  });
