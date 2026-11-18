import { createHash } from "node:crypto";
import { isDeepStrictEqual as equal } from "node:util";

// Reviewed source, never loaded from a candidate manifest, actor or environment.
// This one-use receipt binds actual registry bytes and the real first parent.
const receipt = {
  sourceSha: "3c8e48a2f0f4c345edc836493419184914d3bc27",
  policy: "533633aab653b99790892bf91a1a76155ed59a6ccbe0c12d6f13d19673b527b0",
  before: {
    "package.json":
      "ae2345bce95a9c104a0b9172802f3e6bc85dd8f099001dbc529c2bc66c24f7dc",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "316c3b36ae6117f6750580b72666641a0f4e73c9aa508016886d846871c607d0",
  },
  after: {
    "package.json":
      "97d3086a92d38115d8a51a5f762ffdabac78666f4591885ba93ab933bf77eb04",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "71443627c3a873a63359bab30c82e875784fab3569dee7921e288665bb122d99",
  },
  baseDocuments: {
    "spec/src/current/governance/dependency-automation.md":
      "e36120b374e368eb95481f7e0a6c0ca679646786f604cc43fddb4770a5010681",
    "spec/src/current/governance/dependency-toolchain.md":
      "2999e9e57665da642879b33c29672e9d7a9ba274f9adfe90c592afadd3a533ba",
  },
  documents: {
    "spec/src/current/governance/dependency-automation.md":
      "03fdf21bcc794a9464f756ce5161f6d67354f7c06f8ab1bd7ce07eea9bc60965",
    "spec/src/current/governance/dependency-toolchain.md":
      "7169c1c3aefef05b8cf2a8b279a202f763f0d9ab835047e16e42e59ed0f823ec",
  },
  paths: [
    ".changeset/lapis-dependencies-3203d504ee84.md",
    "package.json",
    "pnpm-lock.yaml",
    "scripts/dependency-automation/check-workspace-intake.mjs",
    "scripts/dependency-automation/reviewed-tasks-transition.mjs",
    "scripts/dependency-automation/reviewed-tasks-transition.test.mjs",
    "scripts/dependency-automation/workspace-catalog.mjs",
    "scripts/dependency-automation/workspace-intake.mjs",
    "scripts/fixtures/reviewed-tasks-transition.json.gz",
    "spec/records/2026-11-18-reviewed-tasks-transition.md",
    "spec/src/backlog/qualification.md",
    "spec/src/current/governance.md",
    "spec/src/current/governance/dependency-automation.md",
    "spec/src/current/governance/dependency-toolchain.md",
  ],
};
const hash = (value) => createHash("sha256").update(value).digest("hex");
export function reviewedTasksTransition(data) {
  if (
    !data.changedFiles.includes(
      "scripts/dependency-automation/reviewed-tasks-transition.mjs",
    )
  )
    return null;
  const fail = () => {
    throw Error("Reviewed Tasks source transition binding mismatch");
  };
  if (
    data.sourceSha !== receipt.sourceSha ||
    hash(data.policyBytes) !== receipt.policy ||
    hash(data.basePolicyBytes) !== receipt.policy ||
    !equal([...data.changedFiles].sort(), receipt.paths)
  )
    fail();
  for (const [field, values] of Object.entries({
    before: receipt.before,
    files: receipt.after,
    baseDocuments: receipt.baseDocuments,
    documents: receipt.documents,
  })) {
    if (!data[field]) fail();
    for (const [path, digest] of Object.entries(values))
      if (
        typeof data[field][path] !== "string" ||
        hash(data[field][path]) !== digest
      )
        fail();
  }
  return { paths: receipt.paths };
}
