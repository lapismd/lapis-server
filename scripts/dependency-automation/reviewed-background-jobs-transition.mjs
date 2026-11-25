import { createHash } from "node:crypto";
import { isDeepStrictEqual as equal } from "node:util";

// Reviewed source, never loaded from a candidate manifest, actor or environment.
// The immutable receipt is populated after the complete candidate is assembled.
const receipt = {
  sourceSha: "981dc3d0da9a8097cb2adce1a12099588339df15",
  policy: "9becfe19edbde0008a83464e72fbd4c7d27a4a29c741c00c86c1ae533a818afe",
  before: {
    "package.json":
      "88f1b00e075c55afa1477230a6558eb35d64d30bae46987649bc787fb2897a4e",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "a6b5d9ae900045b5043645a6b073c904947818c96cf8f37fee4832238402b23f",
  },
  after: {
    "package.json":
      "11a6256166e58bd87223aef635b5227802881e199863bfac14190d2837103eef",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "30cf3af6a969514ac844e4959cc1eeb47d78113aa482ee6afd15c7e7d6fafeb3",
  },
  baseDocuments: {
    "spec/src/current/governance/dependency-automation.md":
      "11afefa8d61a2add1e11950297b8a253b2a960cfd4847d685e62050baee14ead",
    "spec/src/current/governance/dependency-toolchain.md":
      "c4fc3e4f69dbb4513b45bab3b33cc10e66ac1b0b7a7e64488bf0cc3c104af28a",
  },
  documents: {
    "spec/src/current/governance/dependency-automation.md":
      "8b96a76d1e1f7a7f11d7010ee66fec8b42d9a16dd8f6fe0d3182fad07b5bcaa8",
    "spec/src/current/governance/dependency-toolchain.md":
      "9f7e2473aa2ab981181202b3df9cb959da605461182ebb2c6477ac31b8a8c31d",
  },
  paths: [
    "package.json",
    "pnpm-lock.yaml",
    "scripts/dependency-automation/reviewed-background-jobs-transition.mjs",
    "scripts/dependency-automation/reviewed-background-jobs-transition.test.mjs",
    "scripts/dependency-automation/workspace-catalog.mjs",
    "scripts/dependency-automation/workspace-intake.mjs",
    "scripts/fixtures/reviewed-background-jobs-transition.json.gz",
    "spec/records/2026-11-25-background-registry-adoption.md",
    "spec/src/current/governance.md",
    "spec/src/current/governance/dependency-automation.md",
    "spec/src/current/governance/dependency-toolchain.md",
    "web/e2e/hosted-notes-ui.spec.ts",
  ],
};

const hash = (value) => createHash("sha256").update(value).digest("hex");

export function reviewedBackgroundJobsTransition(data) {
  if (
    !data.changedFiles.includes(
      "scripts/dependency-automation/reviewed-background-jobs-transition.mjs",
    )
  )
    return null;
  const fail = () => {
    throw Error("Reviewed background-service transition binding mismatch");
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
  return { paths: receipt.paths, backgroundJobsTransition: true };
}
