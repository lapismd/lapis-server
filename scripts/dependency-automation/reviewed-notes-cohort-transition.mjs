import { createHash } from "node:crypto";
import { isDeepStrictEqual as equal } from "node:util";

// Reviewed source, never loaded from a candidate manifest, actor or environment.
// The immutable receipt is populated after the complete candidate is assembled.
const receipt = {
  sourceSha: "e4dfc2e96943fcaf3c8afdc6b51f851acf4c500d",
  policy: "9becfe19edbde0008a83464e72fbd4c7d27a4a29c741c00c86c1ae533a818afe",
  before: {
    "package.json":
      "1709427c406f63d549b5a47740a28e1720d3b307ae059cabab345b796d89a591",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "201755d4d9cd96086a14965810b27916c0fff95a584e822d34262edab58034cf",
  },
  after: {
    "package.json":
      "3cd6d1540efa44177a8bc4ea71c00ff5b1daac66666e308c58aa971a8fea36a1",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "16d27197ccc0c5617edf826662d27b484710d7690f4be209122d00af86ceb226",
  },
  baseDocuments: {
    "spec/src/current/governance/dependency-automation.md":
      "e0e8b205be0bbcc76d08255d0c55ea762936e5b94250c3f61337afd0217c4909",
    "spec/src/current/governance/dependency-toolchain.md":
      "a2ac4b22fa737eaabaee44e4cbb07c3a35ad966816011d5b69881436d6e0b299",
  },
  documents: {
    "spec/src/current/governance/dependency-automation.md":
      "d05a435f4211d86e40bb7e078d288be16d2019a35f0c2a67b5991d9f8bb8a968",
    "spec/src/current/governance/dependency-toolchain.md":
      "0470a153fb293718c4e855bcf440a6320c9f54d7016d81cf121005d03730cbfc",
  },
  paths: [
    "package.json",
    "pnpm-lock.yaml",
    "scripts/dependency-automation/reviewed-notes-cohort-transition.mjs",
    "scripts/dependency-automation/reviewed-notes-cohort-transition.test.mjs",
    "scripts/dependency-automation/source-intake-reader.mjs",
    "scripts/dependency-automation/source-intake-reader.test.mjs",
    "scripts/dependency-automation/workspace-catalog.mjs",
    "scripts/dependency-automation/workspace-intake.mjs",
    "scripts/fixtures/reviewed-notes-cohort-transition.json.gz",
    "spec/records/2026-11-20-notes-cohort-registry-alignment.md",
    "spec/src/current/governance.md",
    "spec/src/current/governance/dependency-automation.md",
    "spec/src/current/governance/dependency-toolchain.md",
  ],
};

const hash = (value) => createHash("sha256").update(value).digest("hex");

export function reviewedNotesCohortTransition(data) {
  if (
    !data.changedFiles.includes(
      "scripts/dependency-automation/reviewed-notes-cohort-transition.mjs",
    )
  )
    return null;
  const fail = () => {
    throw Error("Reviewed Notes cohort transition binding mismatch");
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
  return { paths: receipt.paths, notesCohortTransition: true };
}
