import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync, mkdtempSync, mkdirSync, rmSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { createHash } from "node:crypto";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { sourceIntakeReader } from "./source-intake-reader.mjs";
import { verifyValidatorCanonicalIntake } from "./legacy-validator-fixture.mjs";

const frozenBytes = readFileSync(
  new URL("../fixtures/validator-intake.json.gz", import.meta.url),
);
const sdk = JSON.parse(gunzipSync(frozenBytes));
const generated = JSON.parse(
  gunzipSync(
    readFileSync(
      new URL("../fixtures/validator-generated-scope.json.gz", import.meta.url),
    ),
  ),
);
assert.equal(
  createHash("sha256").update(frozenBytes).digest("hex"),
  generated.sdkFixtureSha256,
);
const parent = "a".repeat(40); // Controlled SDK fixture anchor, not a production candidate.
function input(hasJj = false) {
  const commands = [];
  const marker = hasJj ? mkdtempSync(join(tmpdir(), "server-jj-scope-")) : null;
  if (marker) mkdirSync(join(marker, ".jj"));
  const execute = (binary, args) => {
    commands.push([binary, ...args]);
    assert.equal(binary, hasJj ? "jj" : "git");
    if (args.includes("log") || args[0] === "rev-parse") return parent;
    if (args.includes("show"))
      return sdk.before[binary === "git" ? args[1].slice(41) : args.at(-1)];
    assert(
      args.includes(parent),
      "file scope must use the captured fact baseline",
    );
    assert(args.includes(hasJj ? "@" : "HEAD"));
    assert(args.includes("diff"));
    return generated.gitDiff;
  };
  const reader = sourceIntakeReader(
    marker ?? "/controlled-server-git-fixture",
    execute,
    hasJj,
  );
  let changedFiles;
  try {
    assert.equal(reader("package.json"), sdk.before["package.json"]);
    changedFiles = reader.changedFiles();
  } finally {
    if (marker) rmSync(marker, { recursive: true, force: true });
  }
  assert.equal(reader.sourceSha, parent);
  assert.deepEqual(
    [...changedFiles].sort(),
    [...generated.changedFiles].sort(),
  );
  return {
    commands,
    data: {
      before: structuredClone(sdk.before),
      files: {
        ...sdk.after,
        [generated.notePath]: generated.noteBytes,
      },
      policyBytes: generated.policyBytes,
      basePolicyBytes: generated.policyBytes,
      baseDocuments: structuredClone(generated.baseDocuments),
      documents: structuredClone(generated.documents),
      changedFiles,
    },
  };
}
for (const hasJj of [false, true])
  test(`clean ${hasJj ? "Jujutsu" : "Git"} range accepts real generated empty-note output`, () => {
    const { data, commands } = input(hasJj);
    const saved = structuredClone(data);
    assert.equal(verifyValidatorCanonicalIntake(data).version, "0.2.1");
    assert(data.changedFiles.includes(generated.notePath));
    assert(commands.at(-1).includes(parent));
    assert.deepEqual(data, saved);
  });
for (const [name, mutate, message] of [
  [
    "runtime intent",
    (d) =>
      (d.files[generated.notePath] =
        '---\n"@lapismd/lapis-server": patch\n---\n'),
    /empty managed maintenance note/,
  ],
  [
    "null intent",
    (d) => (d.files[generated.notePath] = "---\nnull\n---\n"),
    /empty managed maintenance note/,
  ],
  [
    "oversized UTF-8 bytes",
    (d) => (d.files[generated.notePath] = "---\n---\n" + "é".repeat(4096)),
    /bounded managed note bytes/,
  ],
  [
    "ill-formed note text",
    (d) => (d.files[generated.notePath] += "\ud800"),
    /bounded managed note bytes/,
  ],
  [
    "unknown tracked implementation",
    (d) => d.changedFiles.push("web/src/unreviewed.ts"),
    /Unexpected development intake source scope/,
  ],
  [
    "unrelated catalog setting",
    (d) => (d.files["pnpm-lock.yaml"] += "\nunknownSetting: true\n"),
    /registry catalog change/,
  ],
])
  test(`anchored scope rejects ${name} without mutation`, () => {
    const { data } = input();
    mutate(data);
    const saved = structuredClone(data);
    assert.throws(() => verifyValidatorCanonicalIntake(data), message);
    assert.deepEqual(data, saved);
  });
test("the exact 8192-byte empty note boundary remains accepted", () => {
  const { data } = input();
  data.files[generated.notePath] = "---\n---\n" + "é".repeat(4092);
  assert.equal(Buffer.byteLength(data.files[generated.notePath]), 8192);
  assert.equal(verifyValidatorCanonicalIntake(data).version, "0.2.1");
});
test("missing captured Git ancestry cannot fall back to a working-tree diff", () => {
  assert.throws(
    () => sourceIntakeReader("/controlled-server-git-fixture", () => "", false),
    /Source intake ancestry unavailable/,
  );
});
