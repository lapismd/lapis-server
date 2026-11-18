import { createHash } from "node:crypto";
import { isDeepStrictEqual as equal } from "node:util";

// Reviewed source, never loaded from a candidate manifest, actor or environment.
// The immutable receipt is populated after the complete candidate is assembled.
const receipt = {
  sourceSha: "bbc395ee26efe99cfe6f1454d2abd00f719d9743",
  policy: "9becfe19edbde0008a83464e72fbd4c7d27a4a29c741c00c86c1ae533a818afe",
  basePolicy:
    "533633aab653b99790892bf91a1a76155ed59a6ccbe0c12d6f13d19673b527b0",
  before: {
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "71443627c3a873a63359bab30c82e875784fab3569dee7921e288665bb122d99",
    "package.json":
      "97d3086a92d38115d8a51a5f762ffdabac78666f4591885ba93ab933bf77eb04",
  },
  after: {
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "7096c30dc865519c918cb94eab868ab79dd0f5ff36e31eaf09ae0813e73a2de3",
    "package.json":
      "0c46c993991ed76a4f39b12b83dd0541e813a490700aabb9318ff8c02e04158b",
    ".changeset/lapis-dependencies-a3d8c7e0f541.md":
      "3f6899db905dcd77c8787d3f62bd5ddab37db1abe3db991b94cf83263c7035bb",
  },
  baseDocuments: {
    "spec/src/current/governance/dependency-automation.md":
      "03fdf21bcc794a9464f756ce5161f6d67354f7c06f8ab1bd7ce07eea9bc60965",
    "spec/src/current/governance/dependency-toolchain.md":
      "7169c1c3aefef05b8cf2a8b279a202f763f0d9ab835047e16e42e59ed0f823ec",
  },
  documents: {
    "spec/src/current/governance/dependency-automation.md":
      "7afde68a5940485e7e3f191e0c4ad1f2a6063f6e77b3491543c11e39061231cd",
    "spec/src/current/governance/dependency-toolchain.md":
      "2cdd7b502b4226d25c1b1d759a9d1b2135091df7daa7bf355581f6dc93f5daa1",
  },
  paths: [
    ".changeset/lapis-dependencies-a3d8c7e0f541.md",
    ".github/dependency-automation/canonical-intake.json",
    "package.json",
    "pnpm-lock.yaml",
    "scripts/dependency-automation/check-workspace-intake.mjs",
    "scripts/dependency-automation/consumption-fixture.mjs",
    "scripts/dependency-automation/reviewed-terminal-transition.mjs",
    "scripts/dependency-automation/reviewed-terminal-transition.test.mjs",
    "scripts/dependency-automation/workspace-catalog.mjs",
    "scripts/dependency-automation/workspace-intake.mjs",
    "scripts/fixtures/reviewed-terminal-transition.json.gz",
    "spec/records/2026-11-18-hosted-terminal-registry-intake.md",
    "spec/src/current/governance.md",
    "spec/src/current/governance/dependency-automation.md",
    "spec/src/current/governance/dependency-toolchain.md",
    "spec/src/verification.md",
    "web/e2e/hosted-terminal.spec.ts",
    "web/playwright.hosted-notes.config.ts",
    "web/renderer-dependencies.ts",
    "web/src/hosted/rust/backend.ts",
    "web/src/hosted/rust/terminal-client.test.ts",
    "web/src/hosted/rust/terminal-client.ts",
    "web/src/hosted/rust/terminal-registration.test.ts",
    "web/src/hosted/rust/terminal-registration.ts",
    "web/src/hosted/rust/terminal-stream.ts",
  ],
};

const hash = (value) => createHash("sha256").update(value).digest("hex");

export function reviewedTerminalTransition(data) {
  if (
    !data.changedFiles.includes(
      "scripts/dependency-automation/reviewed-terminal-transition.mjs",
    )
  )
    return null;
  const fail = () => {
    throw Error("Reviewed Terminal source transition binding mismatch");
  };
  if (
    data.sourceSha !== receipt.sourceSha ||
    hash(data.policyBytes) !== receipt.policy ||
    hash(data.basePolicyBytes) !== receipt.basePolicy ||
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
  return {
    paths: receipt.paths,
    added: "@lapis-notes/lapis-plugin-terminal",
  };
}
