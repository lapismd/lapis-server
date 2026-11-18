import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import YAML from "yaml";
import { verifyIntake } from "./workspace-intake.mjs";
const fixture = JSON.parse(
  gunzipSync(
    readFileSync(
      new URL("../fixtures/reviewed-tasks-transition.json.gz", import.meta.url),
    ),
  ),
);
const fresh = () => structuredClone(fixture);
test("real registry Tasks transition admits exact immutable source only", () => {
  const d = fresh(),
    saved = structuredClone(d);
  const facts = verifyIntake(d);
  assert.equal(
    facts.manifests["package.json"].devDependencies[
      "@lapis-notes/lapis-plugin-tasks"
    ].version,
    "0.2.1",
  );
  assert.deepEqual(d, saved);
});
test("ordinary attempted Tasks minor remains rejected without source receipt", () => {
  const d = fresh();
  d.changedFiles = [
    "package.json",
    "pnpm-lock.yaml",
    ...Object.keys(d.documents),
  ];
  assert.throws(() => verifyIntake(d), /only compatible forward patches/);
});
for (const [name, mutate] of [
  ["wrong parent", (d) => (d.sourceSha = "f".repeat(40))],
  ["reuse at successor", (d) => (d.sourceSha = "a".repeat(40))],
  ["missing parent", (d) => delete d.sourceSha],
  ["wrong policy", (d) => (d.basePolicyBytes += " ")],
  ["wrong before", (d) => (d.before["pnpm-lock.yaml"] += "\n")],
  ["wrong after", (d) => (d.files["pnpm-lock.yaml"] += "\n")],
  ["extra path", (d) => d.changedFiles.push("Cargo.toml")],
  [
    "missing reviewed implementation",
    (d) =>
      (d.changedFiles = d.changedFiles.filter(
        (p) => !p.endsWith("workspace-catalog.mjs"),
      )),
  ],
  [
    "extra advisory",
    (d) => {
      const m = JSON.parse(d.files["package.json"]);
      m.pnpm.auditConfig.ignoreCves.push("unreviewed");
      d.files["package.json"] = JSON.stringify(m);
    },
  ],
  [
    "extra Tasks metadata",
    (d) => {
      const l = YAML.parse(d.files["pnpm-lock.yaml"]);
      l.packages["@lapis-notes/lapis-plugin-tasks@0.2.1"].deprecated =
        "unreviewed";
      d.files["pnpm-lock.yaml"] = YAML.stringify(l);
    },
  ],
  [
    "unrelated immutable metadata",
    (d) => {
      const l = YAML.parse(d.files["pnpm-lock.yaml"]);
      l.packages["@lapis-notes/api@0.6.5"].deprecated = "unreviewed";
      d.files["pnpm-lock.yaml"] = YAML.stringify(l);
    },
  ],
  [
    "workspace setting",
    (d) =>
      (d.files["pnpm-workspace.yaml"] +=
        "\nonlyBuiltDependencies: [unreviewed]\n"),
  ],
  [
    "unreviewed canonical prose",
    (d) => (d.documents[Object.keys(d.documents)[0]] += "unreviewed"),
  ],
  [
    "stale canonical baseline",
    (d) => (d.baseDocuments[Object.keys(d.baseDocuments)[0]] += "unreviewed"),
  ],
  [
    "nonempty note",
    (d) => {
      const p = ".changeset/lapis-dependencies-123456789abc.md";
      d.changedFiles.push(p);
      d.files[p] = '---\n"@lapismd/lapis-server": patch\n---\n\nFake\n';
    },
  ],
])
  test("rejects reviewed transition " + name + " without mutation", () => {
    const d = fresh();
    mutate(d);
    const saved = structuredClone(d);
    assert.throws(() => verifyIntake(d));
    assert.deepEqual(d, saved);
  });
