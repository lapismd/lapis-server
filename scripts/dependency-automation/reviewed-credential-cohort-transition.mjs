import { createHash } from "node:crypto";
import { isDeepStrictEqual as equal } from "node:util";

// Only the exact reviewed registry graph can change Notes cohort metadata.
const receipt = {
  sourceSha: "b24a1914ec6d46dd19714f75d732bd0a71fd63da",
  policy: "9becfe19edbde0008a83464e72fbd4c7d27a4a29c741c00c86c1ae533a818afe",
  before: {
    "package.json":
      "11a6256166e58bd87223aef635b5227802881e199863bfac14190d2837103eef",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "30cf3af6a969514ac844e4959cc1eeb47d78113aa482ee6afd15c7e7d6fafeb3",
  },
  after: {
    "package.json":
      "9a91affc602e964745fe0923ac7ed936ed297315e52718b7504bf3cb99a31532",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "3cd8e074c0c05c585d9957c21e68621b1471bfb8dd11ef97dfa2a2397055e1a7",
  },
  baseDocuments: {
    "spec/src/current/governance/dependency-automation.md":
      "8b96a76d1e1f7a7f11d7010ee66fec8b42d9a16dd8f6fe0d3182fad07b5bcaa8",
    "spec/src/current/governance/dependency-toolchain.md":
      "9f7e2473aa2ab981181202b3df9cb959da605461182ebb2c6477ac31b8a8c31d",
  },
  documents: {
    "spec/src/current/governance/dependency-automation.md":
      "fc7c9b5df96d5984fd364602da34bda8a81cc5da3a5428ebdbf89d6600862db6",
    "spec/src/current/governance/dependency-toolchain.md":
      "b5a8faedb24c02eced42f28b1395411790a47af70d89115c84ef7a649184decf",
  },
  paths: [
    "package.json",
    "pnpm-lock.yaml",
    "scripts/dependency-automation/reviewed-credential-cohort-transition.mjs",
    "scripts/dependency-automation/reviewed-credential-cohort-transition.test.mjs",
    "scripts/dependency-automation/workspace-intake.mjs",
    "scripts/fixtures/reviewed-credential-cohort-transition.json.gz",
    "spec/records/2026-11-25-credential-package-intake.md",
    "spec/src/current/governance.md",
    "spec/src/current/governance/dependency-automation.md",
    "spec/src/current/governance/dependency-toolchain.md",
  ],
};

const hash = (value) => createHash("sha256").update(value).digest("hex");

export function reviewedCredentialCohortTransition(data) {
  if (
    !data.changedFiles.includes(
      "scripts/dependency-automation/reviewed-credential-cohort-transition.mjs",
    )
  )
    return null;
  const fail = () => {
    throw Error("Reviewed credential cohort transition binding mismatch");
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
