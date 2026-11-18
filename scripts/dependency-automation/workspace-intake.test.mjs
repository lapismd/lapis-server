import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import YAML from "yaml";
import { intakeFacts, intakeSlot, verifyIntake } from "./workspace-intake.mjs";
const fixture = JSON.parse(
  gunzipSync(
    readFileSync(
      new URL("../fixtures/workspace-intake.json.gz", import.meta.url),
    ),
  ),
);
const fresh = () => structuredClone(fixture.core);
const replace = (data, key, value) =>
  (data.files[key] = typeof value === "string" ? value : YAML.stringify(value));
const policy = JSON.parse(fixture.core.policyBytes);
const note = ".changeset/lapis-dependencies-6882649476db.md";
const emptyNote =
  "---\n---\n\nRefresh first-party Lapis dependencies to the published versions recorded in this pull request.\n";
function syncDocuments(data) {
  const facts = intakeFacts(JSON.parse(data.policyBytes), data.files);
  for (const path of policy.chapters)
    data.documents[path] = "# Source\n\n" + intakeSlot(policy, facts) + "\n";
}
test("actual native published root Core0.13.4→0.13.5 has no private product intent", () => {
  const d = fresh(),
    saved = structuredClone(d);
  const facts = verifyIntake(d);
  assert.equal(
    facts.manifests["package.json"].dependencies["@lapismd/design-core"]
      .version,
    "0.13.5",
  );
  assert.equal(JSON.parse(d.files["package.json"]).version, "2.4.0");
  assert.deepEqual(d, saved);
});
test("one actual managed empty note accompanies private Core intake", () => {
  const d = fresh();
  d.changedFiles.push(note);
  d.files[note] = emptyNote;
  verifyIntake(d);
});
test("current source facts require no synthetic update or note", () => {
  const d = fresh();
  d.files = structuredClone(d.before);
  d.documents = structuredClone(d.baseDocuments);
  d.changedFiles = [];
  verifyIntake(d);
});
test("actual legacy SDK0.2.0→0.2.1 pair retains exact override and empty intent", () => {
  const d = fresh();
  d.before = structuredClone(fixture.sdk.before);
  d.files = structuredClone(fixture.sdk.after);
  d.baseDocuments = Object.fromEntries(
    policy.chapters.map((p) => [
      p,
      "# Source\n\n" + intakeSlot(policy, intakeFacts(policy, d.before)) + "\n",
    ]),
  );
  syncDocuments(d);
  d.changedFiles.push(note);
  d.files[note] = emptyNote;
  assert.equal(
    verifyIntake(d).manifests["package.json"].devDependencies[
      "@lapismd/spec-validator"
    ].version,
    "0.2.1",
  );
});
for (const [name, mutate] of [
  [
    "stale current facts",
    (d) =>
      (d.documents[policy.chapters[0]] = d.baseDocuments[policy.chapters[0]]),
  ],
  ["missing slot", (d) => (d.documents[policy.chapters[0]] = "none")],
  [
    "duplicate slot",
    (d) => (d.documents[policy.chapters[0]] += d.documents[policy.chapters[0]]),
  ],
  ["manual prose", (d) => (d.documents[policy.chapters[0]] += "Unreviewed")],
  ["stale source policy", (d) => (d.basePolicyBytes += " ")],
  [
    "foreign repository",
    (d) => {
      const p = JSON.parse(d.policyBytes);
      p.repository = "lapismd/other";
      d.policyBytes = JSON.stringify(p);
    },
  ],
  [
    "removed owning chapter",
    (d) => {
      const p = JSON.parse(d.policyBytes);
      p.chapters.pop();
      d.policyBytes = JSON.stringify(p);
    },
  ],
  [
    "added dependency capability",
    (d) => {
      const p = JSON.parse(d.policyBytes);
      p.dependencies.push("@lapismd/ai");
      d.policyBytes = JSON.stringify(p);
    },
  ],
  [
    "unexpected code",
    (d) => d.changedFiles.push("crates/lapis-server/src/main.rs"),
  ],
  [
    "unbounded note",
    (d) => {
      d.changedFiles.push(note);
      d.files[note] = emptyNote + "é".repeat(4096);
    },
  ],
  [
    "ill-formed note",
    (d) => {
      d.changedFiles.push(note);
      d.files[note] = emptyNote + "\ud800";
    },
  ],
  [
    "nonempty private release intent",
    (d) => {
      d.changedFiles.push(note);
      d.files[note] =
        '---\n"@lapismd/lapis-server": patch\n---\n\nFake version\n';
    },
  ],
  [
    "null intent",
    (d) => {
      d.changedFiles.push(note);
      d.files[note] = "---\nnull\n---\n\nInvalid\n";
    },
  ],
  [
    "multiple managed notes",
    (d) => {
      d.changedFiles.push(
        note,
        ".changeset/lapis-dependencies-123456789abc.md",
      );
      d.files[note] = emptyNote;
      d.files[d.changedFiles.at(-1)] = emptyNote;
    },
  ],
  [
    "arbitrary note",
    (d) => {
      d.changedFiles.push(".changeset/manual.md");
      d.files[".changeset/manual.md"] = emptyNote;
    },
  ],
  ["missing managed bytes", (d) => d.changedFiles.push(note)],
  [
    "private identity",
    (d) => {
      const m = JSON.parse(d.files["package.json"]);
      m.private = false;
      replace(d, "package.json", JSON.stringify(m));
    },
  ],
  [
    "product version",
    (d) => {
      const m = JSON.parse(d.files["package.json"]);
      m.version = "2.4.1";
      replace(d, "package.json", JSON.stringify(m));
    },
  ],
  [
    "runtime script",
    (d) => {
      const m = JSON.parse(d.files["package.json"]);
      m.scripts.build = "fake";
      replace(d, "package.json", JSON.stringify(m));
    },
  ],
  ["native manifest", (d) => d.changedFiles.push("Cargo.toml")],
  [
    "external resolution",
    (d) => {
      const l = YAML.parse(d.files["pnpm-lock.yaml"]);
      l.packages["@lapismd/design-core@0.13.5"].resolution.tarball =
        "https://other.test/fake.tgz";
      replace(d, "pnpm-lock.yaml", l);
    },
  ],
  [
    "unrelated importer",
    (d) => {
      const l = YAML.parse(d.files["pnpm-lock.yaml"]);
      l.importers["."].devDependencies.prettier.version = "3.6.3";
      replace(d, "pnpm-lock.yaml", l);
    },
  ],
  [
    "same-version API metadata",
    (d) => {
      const l = YAML.parse(d.files["pnpm-lock.yaml"]);
      l.packages["@lapis-notes/api@0.6.5"].engines = { node: ">=99" };
      replace(d, "pnpm-lock.yaml", l);
    },
  ],
  [
    "unrelated first-party identity",
    (d) => {
      const l = YAML.parse(d.files["pnpm-lock.yaml"]);
      l.packages["@lapismd/ai@0.2.0"] = {
        resolution: {
          integrity: "sha512-" + Buffer.alloc(64).toString("base64"),
        },
      };
      replace(d, "pnpm-lock.yaml", l);
    },
  ],
  [
    "first-party snapshot edge",
    (d) => {
      const l = YAML.parse(d.files["pnpm-lock.yaml"]);
      const k = Object.keys(l.snapshots).find((k) =>
        k.startsWith("@lapis-notes/app-profile@"),
      );
      l.snapshots[k].dependencies["@lapis-notes/api"] = "0.6.4";
      replace(d, "pnpm-lock.yaml", l);
    },
  ],
  [
    "workspace setting",
    (d) => {
      const w = YAML.parse(d.files["pnpm-workspace.yaml"]);
      w.onlyBuiltDependencies = ["unreviewed"];
      replace(d, "pnpm-workspace.yaml", w);
    },
  ],
  [
    "validator override",
    (d) => {
      const w = YAML.parse(d.files["pnpm-workspace.yaml"]);
      w.overrides["@lapismd/spec-validator"] = "0.2.0";
      replace(d, "pnpm-workspace.yaml", w);
    },
  ],
  [
    "unknown lock setting",
    (d) => (d.files["pnpm-lock.yaml"] += "\nunknownSetting: true\n"),
  ],
  [
    "Core minor migration",
    (d) => {
      const m = JSON.parse(d.files["package.json"]);
      m.dependencies["@lapismd/design-core"] = "^0.14.0";
      replace(d, "package.json", JSON.stringify(m));
    },
  ],
  [
    "peer addition",
    (d) => {
      const m = JSON.parse(d.files["package.json"]);
      m.peerDependencies = { "@lapismd/design-core": "^0.13.5" };
      replace(d, "package.json", JSON.stringify(m));
    },
  ],
  [
    "file dependency",
    (d) => {
      const m = JSON.parse(d.files["package.json"]);
      m.dependencies["@lapismd/design-core"] = "file:../design-core";
      replace(d, "package.json", JSON.stringify(m));
    },
  ],
])
  test("rejects " + name + " without mutation", () => {
    const d = fresh();
    mutate(d);
    const saved = structuredClone(d);
    assert.throws(() => verifyIntake(d));
    assert.deepEqual(d, saved);
  });
