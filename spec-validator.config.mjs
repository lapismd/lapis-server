import {
  defineConfig,
  headingRequirements,
  singleIdVerification,
} from "@lapismd/spec-validator";

export default defineConfig(headingRequirements(), {
  name: "lapis-server",
  idPattern: /^SRV-[A-Z]+-\d{3}$/,
  ruleIds: {
    governance: "SRV-GOV-001",
    fileLimits: "SRV-GOV-001",
    summary: "SRV-GOV-001",
    verification: "SRV-GOV-001",
    book: "SRV-GOV-001",
    bookIgnore: "SRV-GOV-001",
    specFirst: "SRV-GOV-001",
    internal: "SRV-GOV-001",
  },
  validators: {
    summary: true,
    governance: { extras: ["AGENTS.md", "README.md"] },
    fileLimits: { maxLines: 400, maxBytes: 32768 },
    verification: singleIdVerification(),
    book: true,
    specFirst: {
      mode: "mapped",
      canonicalPattern: "^spec/src/current/(?:server|governance)\\.md$",
      protected: ["^(?:crates/|scripts/|\\.github/|Cargo\\.toml$|Dockerfile$)"],
      ignore: ["(^|/)(?:node_modules|target|spec/book)/"],
      rules: [
        { pattern: "^crates/", chapters: ["spec/src/current/server.md"] },
        { pattern: ".*", chapters: ["spec/src/current/governance.md"] },
      ],
    },
  },
  check: { build: true, first: true },
});
