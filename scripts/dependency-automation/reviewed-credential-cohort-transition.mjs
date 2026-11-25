import { createHash } from "node:crypto";
import { isDeepStrictEqual as equal } from "node:util";

// Only the exact reviewed registry graph can change Notes cohort metadata.
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
      "9a91affc602e964745fe0923ac7ed936ed297315e52718b7504bf3cb99a31532",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "3cd8e074c0c05c585d9957c21e68621b1471bfb8dd11ef97dfa2a2397055e1a7",
  },
  baseDocuments: {
    "spec/src/current/governance/dependency-automation.md":
      "11afefa8d61a2add1e11950297b8a253b2a960cfd4847d685e62050baee14ead",
    "spec/src/current/governance/dependency-toolchain.md":
      "c4fc3e4f69dbb4513b45bab3b33cc10e66ac1b0b7a7e64488bf0cc3c104af28a",
  },
  documents: {
    "spec/src/current/governance/dependency-automation.md":
      "3957ae3bd6e912ea884b356e28b8a198de5ba6148b8a360b80a08e575e26b278",
    "spec/src/current/governance/dependency-toolchain.md":
      "b9899521288efd7150d896c7f9d51a8d7c17b31ddebad7c0c468d7a0b4213999",
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
