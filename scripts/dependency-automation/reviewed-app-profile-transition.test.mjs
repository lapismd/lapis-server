import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import YAML from "yaml";
import { verifyIntake } from "./workspace-intake.mjs";

const fixture = JSON.parse(
  gunzipSync(
    readFileSync(
      new URL("../fixtures/reviewed-app-profile-transition.json.gz", import.meta.url),
    ),
  ),
);
const fresh = () => structuredClone(fixture);

test("real App Profile presentation transition admits exact immutable source only", () => {
  const data = fresh();
  const saved = structuredClone(data);
  const facts = verifyIntake(data);
  assert.equal(
    facts.manifests["package.json"].dependencies["@lapis-notes/app-profile"]
      .version,
    "0.3.0",
  );
  assert.deepEqual(data, saved);
});

test("ordinary canonical patch cannot change App Profile major-minor metadata", () => {
  const data = fresh();
  data.changedFiles = data.changedFiles.filter(
    (path) => !path.endsWith("reviewed-app-profile-transition.mjs"),
  );
  assert.throws(() => verifyIntake(data));
});

for (const [name, mutate] of [
  ["wrong parent", (data) => (data.sourceSha = "f".repeat(40))],
  ["wrong package", (data) => (data.files["package.json"] += "\n")],
  ["wrong lock", (data) => (data.files["pnpm-lock.yaml"] += "\n")],
  ["extra path", (data) => data.changedFiles.push("Cargo.toml")],
  [
    "extra selected metadata",
    (data) => {
      const lock = YAML.parse(data.files["pnpm-lock.yaml"]);
      lock.packages["@lapis-notes/app-profile@0.3.0"].deprecated = "unreviewed";
      data.files["pnpm-lock.yaml"] = YAML.stringify(lock);
    },
  ],
  [
    "unreviewed canonical prose",
    (data) => (data.documents[Object.keys(data.documents)[0]] += "unreviewed"),
  ],
])
  test(`App Profile transition rejects ${name}`, () => {
    const data = fresh();
    mutate(data);
    assert.throws(() => verifyIntake(data));
  });
