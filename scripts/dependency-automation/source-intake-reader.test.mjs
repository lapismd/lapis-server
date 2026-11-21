import assert from "node:assert/strict";
import test from "node:test";
import {
  readFileSync,
  mkdtempSync,
  mkdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
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

test("injected VCS execution ignores ambient pull-request transport", () => {
  const root = mkdtempSync(join(tmpdir(), "server-ambient-pr-base-"));
  const eventPath = join(root, "event.json");
  writeFileSync(
    eventPath,
    JSON.stringify({ pull_request: { base: { sha: "b".repeat(40) } } }),
  );
  const previous = {
    eventName: process.env.GITHUB_EVENT_NAME,
    eventPath: process.env.GITHUB_EVENT_PATH,
  };
  process.env.GITHUB_EVENT_NAME = "pull_request";
  process.env.GITHUB_EVENT_PATH = eventPath;
  const commands = [];
  try {
    const reader = sourceIntakeReader(
      root,
      (binary, args) => {
        commands.push([binary, ...args]);
        if (args[0] === "rev-parse") return parent;
        return generated.gitDiff;
      },
      false,
    );
    assert.equal(reader.sourceSha, parent);
    reader.changedFiles();
  } finally {
    if (previous.eventName === undefined) delete process.env.GITHUB_EVENT_NAME;
    else process.env.GITHUB_EVENT_NAME = previous.eventName;
    if (previous.eventPath === undefined) delete process.env.GITHUB_EVENT_PATH;
    else process.env.GITHUB_EVENT_PATH = previous.eventPath;
    rmSync(root, { recursive: true, force: true });
  }
  assert(commands.some(([, command]) => command === "rev-parse"));
  assert(commands.every(([, ...args]) => !args.includes("b".repeat(40))));
});

test("pull-request intake uses the immutable event base across repair commits", () => {
  const root = mkdtempSync(join(tmpdir(), "server-pr-base-"));
  const eventPath = join(root, "event.json");
  writeFileSync(
    eventPath,
    JSON.stringify({ pull_request: { base: { sha: parent } } }),
  );
  const commands = [];
  const reader = sourceIntakeReader(
    root,
    (binary, args) => {
      commands.push([binary, ...args]);
      if (args[0] === "show") return sdk.before[args[1].slice(41)];
      return generated.gitDiff;
    },
    false,
    { GITHUB_EVENT_NAME: "pull_request", GITHUB_EVENT_PATH: eventPath },
  );
  try {
    assert.equal(reader.sourceSha, parent);
    assert.equal(reader("package.json"), sdk.before["package.json"]);
    reader.changedFiles();
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
  assert(!commands.some(([, command]) => command === "rev-parse"));
  assert(commands.every(([, ...args]) => !args.includes("HEAD^1")));
  assert(commands.some(([, ...args]) => args.includes(parent)));
});
