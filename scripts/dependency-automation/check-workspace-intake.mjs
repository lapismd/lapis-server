import { readFileSync, realpathSync, lstatSync } from "node:fs";
import { resolve } from "node:path";
import { sourceIntakeReader } from "./source-intake-reader.mjs";
import { isDeepStrictEqual as equal } from "node:util";
import { intakeFacts, verifyIntake } from "./workspace-intake.mjs";
export function checkWorkspaceIntake(root = process.cwd()) {
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
  const base = sourceIntakeReader(root);
  const before = Object.fromEntries(inputs.map((p) => [p, base(p)]));
  const changedFiles = base.changedFiles();
  for (const path of changedFiles)
    if (/^\.changeset\/lapis-dependencies-[a-f0-9]{12}\.md$/.test(path))
      files[path] = read(path);
  const changed = !equal(
    intakeFacts(policy, before),
    intakeFacts(policy, files),
  );
  return verifyIntake({
    policyBytes,
    files,
    documents,
    before,
    changedFiles,
    ...(changed
      ? {
          basePolicyBytes: base(policyPath),
          baseDocuments: Object.fromEntries(
            policy.chapters.map((p) => [p, base(p)]),
          ),
        }
      : {}),
  });
}
