import { readFileSync, realpathSync, lstatSync } from "node:fs";
import { resolve } from "node:path";
import { sourceIntakeReader } from "./source-intake-reader.mjs";
import { isDeepStrictEqual as equal } from "node:util";
import { intakeFacts, verifyIntake } from "./workspace-intake.mjs";
import { verifyWorkspaceNotes } from "./workspace-note.mjs";
export function checkWorkspaceIntake(
  root = process.cwd(),
  sourceReader = sourceIntakeReader,
) {
  const read = (path) => {
    const target = resolve(root, path);
    if (realpathSync(target) !== target || !lstatSync(target).isFile())
      throw Error("Non-regular Server intake source");
    return readFileSync(target, "utf8");
  };
  const policyPath = ".github/dependency-automation/canonical-intake.json";
  const policyBytes = read(policyPath),
    policy = JSON.parse(policyBytes);
  const inputs = ["pnpm-workspace.yaml", "pnpm-lock.yaml", ...policy.manifests];
  const files = Object.fromEntries(inputs.map((p) => [p, read(p)]));
  const documents = Object.fromEntries(
    policy.chapters.map((p) => [p, read(p)]),
  );
  const base = sourceReader(root);
  const basePolicyBytes = base(policyPath);
  const basePolicy = JSON.parse(basePolicyBytes);
  const before = Object.fromEntries(inputs.map((p) => [p, base(p)]));
  const changedFiles = base.changedFiles();
  const presentNotes = [];
  for (const path of changedFiles) {
    if (!/^\.changeset\/lapis-dependencies-[a-f0-9]{12}\.md$/.test(path))
      continue;
    const info = lstatSync(resolve(root, path), { throwIfNoEntry: false });
    if (info === undefined) continue;
    if (info.size > 8192) throw Error("Invalid managed dependency note bytes");
    files[path] = read(path);
    presentNotes.push(path);
  }
  // Server's fixed private-root policy permits only empty managed release maps.
  verifyWorkspaceNotes(presentNotes, files, {});
  const changed = !equal(
    intakeFacts(basePolicy, before),
    intakeFacts(policy, files),
  );
  return verifyIntake({
    policyBytes,
    files,
    documents,
    before,
    changedFiles,
    sourceSha: base.sourceSha,
    ...(changed
      ? {
          basePolicyBytes,
          baseDocuments: Object.fromEntries(
            policy.chapters.map((p) => [p, base(p)]),
          ),
        }
      : {}),
  });
}
