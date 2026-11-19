import { createHash } from "node:crypto";
import { isDeepStrictEqual as equal } from "node:util";

// Reviewed source, never loaded from a candidate manifest, actor or environment.
// The immutable receipt is populated after the complete candidate is assembled.
const receipt = {
  sourceSha: "c0b43f5f711aff0328634b283167d314bf5a87e8",
  policy: "9becfe19edbde0008a83464e72fbd4c7d27a4a29c741c00c86c1ae533a818afe",
  before: {
    "package.json":
      "8506d1d80a0c34fee33b063c13a67c484e07dbd1c9e99ad4798460cc0c0a9156",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "7096c30dc865519c918cb94eab868ab79dd0f5ff36e31eaf09ae0813e73a2de3",
  },
  after: {
    "package.json":
      "512c1db5fbdbac47b29bf21149804fda35bf3bc563795c7f4fc1e90b79862be2",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "f3f08030c209cd211769e6867c5e4dcdffee2aa141fdb278e8da30d41cb71d4b",
  },
  baseDocuments: {
    "spec/src/current/governance/dependency-automation.md":
      "7afde68a5940485e7e3f191e0c4ad1f2a6063f6e77b3491543c11e39061231cd",
    "spec/src/current/governance/dependency-toolchain.md":
      "2cdd7b502b4226d25c1b1d759a9d1b2135091df7daa7bf355581f6dc93f5daa1",
  },
  documents: {
    "spec/src/current/governance/dependency-automation.md":
      "3301dfc8a4583377ad47d55b31037c707187f3715c89d21d0c7b9e3c4e486f7d",
    "spec/src/current/governance/dependency-toolchain.md":
      "059546e396828495ce6699ef9ae0b85c1b682ef91c329dea43dcb6199e984bda",
  },
  paths: [
    "package.json",
    "pnpm-lock.yaml",
    "scripts/dependency-automation/reviewed-notes-peer-transition.mjs",
    "scripts/dependency-automation/reviewed-notes-peer-transition.test.mjs",
    "scripts/dependency-automation/workspace-catalog.mjs",
    "scripts/dependency-automation/workspace-intake.mjs",
    "scripts/fixtures/reviewed-notes-peer-transition.json.gz",
    "spec/records/2026-11-19-notes-peer-prerequisite.md",
    "spec/src/backlog/qualification.md",
    "spec/src/current/governance.md",
    "spec/src/current/governance/dependency-automation.md",
    "spec/src/current/governance/dependency-toolchain.md",
  ],
};

const hash = (value) => createHash("sha256").update(value).digest("hex");

export function reviewedNotesPeerTransition(data) {
  if (
    !data.changedFiles.includes(
      "scripts/dependency-automation/reviewed-notes-peer-transition.mjs",
    )
  )
    return null;
  const fail = () => {
    throw Error("Reviewed Notes peer transition binding mismatch");
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
  return { paths: receipt.paths, notesPeerTransition: true };
}
