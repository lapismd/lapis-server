import { createHash } from "node:crypto";
import { isDeepStrictEqual as equal } from "node:util";

// Reviewed source, never loaded from a candidate manifest, actor or environment.
// The immutable receipt is populated after the complete candidate is assembled.
const receipt = {
  sourceSha: "c1a71cefddf58430bb822f7690b25b5fd86f0bc7",
  policy: "9becfe19edbde0008a83464e72fbd4c7d27a4a29c741c00c86c1ae533a818afe",
  before: {
    "package.json":
      "d8716838a8dee0f3fc45bf68d0453411ff76f2631a361d4654a8f285789405ef",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "97dd3fd298321bfb76807a981234213296967fdadf7c07802d7aa2bf0ec49b7c",
  },
  after: {
    "package.json":
      "af10cb70175ee79c31239ccab0d0e8e779f2de7c7b2a7d206aba8244bd4993f2",
    "pnpm-workspace.yaml":
      "bc64c4693f5c606e687b564e60700659ab0e32ca92d3a0a44c2c223e62f20458",
    "pnpm-lock.yaml":
      "44669b2300278b26e34b1f020d5228195eb46c5d8038e48f660668f3ed96c755",
  },
  baseDocuments: {
    "spec/src/current/governance/dependency-automation.md":
      "a24cfaa83194772510d1097aca286082d2e4d8480138a1ec1c7dec089af0165d",
    "spec/src/current/governance/dependency-toolchain.md":
      "860a042c8bb28b5fc5c6f0e1f09be74f13aa09a0c46bf78e9f8d813316596f6e",
  },
  documents: {
    "spec/src/current/governance/dependency-automation.md":
      "240dd871d90a445ec968f754a6b711634585495f12aedd46609496a8353bb036",
    "spec/src/current/governance/dependency-toolchain.md":
      "b07f00bdbc9d40f8307af17a44c190fa5c49f94c10d9215982aa89be1eb46ee2",
  },
  paths: [
    ".changeset/lapis-dependencies-a3f110e03000.md",
    ".changeset/shared-pwa-presentation.md",
    "package.json",
    "pnpm-lock.yaml",
    "scripts/dependency-automation/reviewed-app-profile-transition.mjs",
    "scripts/dependency-automation/reviewed-app-profile-transition.test.mjs",
    "scripts/dependency-automation/workspace-intake.mjs",
    "scripts/fixtures/reviewed-app-profile-transition.json.gz",
    "spec/records/2026-11-22-app-profile-presentation-adoption.md",
    "spec/src/current/governance.md",
    "spec/src/current/governance/dependency-automation.md",
    "spec/src/current/governance/dependency-toolchain.md",
    "web/e2e/rust-server-pwa.spec.ts",
    "web/src/server-host-document.test.ts",
    "web/src/server-host-document.ts",
  ],
};

const hash = (value) => createHash("sha256").update(value).digest("hex");

export function reviewedAppProfileTransition(data) {
  if (
    !data.changedFiles.includes(
      "scripts/dependency-automation/reviewed-app-profile-transition.mjs",
    )
  )
    return null;
  const fail = () => {
    throw Error("Reviewed App Profile transition binding mismatch");
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
  return { paths: receipt.paths, appProfileTransition: true };
}
