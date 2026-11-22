import { createHash } from "node:crypto";
import { isDeepStrictEqual as equal } from "node:util";

// Reviewed source, never loaded from a candidate manifest, actor or environment.
// This one-use receipt binds actual registry bytes and the real first parent.
const receipt = {
  sourceSha: "3ee32252158532f975de95ab631106d8ba585efb",
  policy: "9becfe19edbde0008a83464e72fbd4c7d27a4a29c741c00c86c1ae533a818afe",
  before: {
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "44669b2300278b26e34b1f020d5228195eb46c5d8038e48f660668f3ed96c755",
    "package.json":
      "4d24fd2f73804b4ae998364803a5a403738b2f579b63018de8aefac9592debdd",
  },
  after: {
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "9f1446f7e95766864670cb27330bc4a861f346c14493f8582decfffb688027fb",
    "package.json":
      "1e0322b12a556a317934a5a126423fad5296893bf6be11fb63f0e4877345bbdf",
  },
  baseDocuments: {
    "spec/src/current/governance/dependency-automation.md":
      "240dd871d90a445ec968f754a6b711634585495f12aedd46609496a8353bb036",
    "spec/src/current/governance/dependency-toolchain.md":
      "b07f00bdbc9d40f8307af17a44c190fa5c49f94c10d9215982aa89be1eb46ee2",
  },
  documents: {
    "spec/src/current/governance/dependency-automation.md":
      "a394e0a68559116bbcc46cb6c673235a055651b76e4c0da8659a0f92b75df004",
    "spec/src/current/governance/dependency-toolchain.md":
      "6fd699116ce090bf05f32d0e3c56fea83828b58a2509aee0bf15664d91e16ca3",
  },
  paths: [
    "package.json",
    "pnpm-lock.yaml",
    "scripts/dependency-automation/reviewed-tasks-mcp-app-transition.mjs",
    "scripts/dependency-automation/reviewed-tasks-mcp-app-transition.test.mjs",
    "scripts/dependency-automation/workspace-catalog.mjs",
    "scripts/dependency-automation/workspace-intake.mjs",
    "scripts/fixtures/reviewed-tasks-mcp-app-transition.json.gz",
    "spec/src/current/governance.md",
    "spec/src/current/governance/dependency-automation.md",
    "spec/src/current/governance/dependency-toolchain.md",
    "web/e2e/rust-server-tasks-recovery.spec.ts",
  ],
};
const hash = (value) => createHash("sha256").update(value).digest("hex");
export function reviewedTasksMcpAppTransition(data) {
  if (
    !data.changedFiles.includes(
      "scripts/dependency-automation/reviewed-tasks-mcp-app-transition.mjs",
    )
  )
    return null;
  const fail = () => {
    throw Error("Reviewed Tasks MCP App transition binding mismatch");
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
  return { paths: receipt.paths, tasksMcpAppTransition: true };
}
