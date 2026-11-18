import {
  readFileSync,
  realpathSync,
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  symlinkSync,
} from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";
import { gunzipSync } from "node:zlib";
import { sourceIntakeReader } from "./source-intake-reader.mjs";

const fixture = JSON.parse(
  gunzipSync(
    readFileSync(
      new URL("../fixtures/workspace-intake.json.gz", import.meta.url),
    ),
  ),
).core;
export const notePath = ".changeset/lapis-dependencies-6882649476db.md";
export const noteBytes =
  "---\n---\n\nRefresh first-party Lapis dependencies to the published versions recorded in this pull request.\n";
const policyPath = ".github/dependency-automation/canonical-intake.json";
const parent = "a".repeat(40); // Controlled transport anchor, never a production fallback.
const deletion =
  `diff --git a/${notePath} b/${notePath}\ndeleted file mode 100644\n--- a/${notePath}\n+++ /dev/null\n@@ -1,4 +0,0 @@\n` +
  noteBytes
    .trimEnd()
    .split("\n")
    .map((line) => "-" + line)
    .join("\n") +
  "\n";

export function consumptionFixture({ changed = false, hasJj = false } = {}) {
  const root = realpathSync(
    mkdtempSync(join(tmpdir(), "server-note-consumption-")),
  );
  mkdirSync(join(root, ".changeset"));
  const data = structuredClone(fixture);
  const current = changed ? data.files : data.before;
  const documents = changed ? data.documents : data.baseDocuments;
  const write = (path, bytes) => {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), bytes);
  };
  for (const [path, bytes] of Object.entries({
    ...current,
    ...documents,
    [policyPath]: data.policyBytes,
  }))
    write(path, bytes);
  if (hasJj) mkdirSync(join(root, ".jj"));
  const before = {
    ...data.before,
    ...data.baseDocuments,
    [policyPath]: data.policyBytes,
    [notePath]: noteBytes,
  };
  let diff = deletion;
  for (const [path, bytes] of Object.entries({ ...current, ...documents })) {
    if (before[path] === bytes) continue;
    const oldLines = before[path].trimEnd().split("\n"),
      newLines = bytes.trimEnd().split("\n");
    diff +=
      `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1,${oldLines.length} +1,${newLines.length} @@\n` +
      oldLines
        .map((line) => "-" + line)
        .concat(newLines.map((line) => "+" + line))
        .join("\n") +
      "\n";
  }
  return {
    root,
    data,
    write,
    sourceReader(directory) {
      return sourceIntakeReader(
        directory,
        (binary, args) => {
          if (args.includes("log") || args[0] === "rev-parse") return parent;
          if (args.includes("show"))
            return before[binary === "git" ? args[1].slice(41) : args.at(-1)];
          if (
            !args.includes(parent) ||
            !args.includes(hasJj ? "@" : "HEAD") ||
            !args.includes("diff")
          )
            throw Error("Unbound test VCS call");
          return diff;
        },
        hasJj,
      );
    },
    present(text = noteBytes) {
      write(notePath, text);
    },
    consumeWithActualCli() {
      write(
        ".changeset/config.json",
        readFileSync(new URL("../../.changeset/config.json", import.meta.url)),
      );
      symlinkSync(
        new URL("../../node_modules", import.meta.url).pathname,
        join(root, "node_modules"),
        "dir",
      );
      write(notePath, noteBytes);
      return execFileSync(
        process.execPath,
        [
          new URL("../../node_modules/@changesets/cli/bin.js", import.meta.url)
            .pathname,
          "version",
        ],
        { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
      );
    },
    externalChangedPath(path) {
      diff += `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -0,0 +1 @@\n+untrusted\n`;
    },
  };
}
