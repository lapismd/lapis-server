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
    qmd: "SRV-GOV-001",
    summary: "SRV-GOV-001",
    verification: "SRV-GOV-001",
    book: "SRV-GOV-001",
    bookIgnore: "SRV-GOV-001",
    specFirst: "SRV-GOV-001",
    internal: "SRV-GOV-001",
    storybookMirrors: "SRV-GOV-002",
  },
  validators: {
    summary: true,
    governance: { extras: ["AGENTS.md", "README.md"] },
    fileLimits: { maxLines: 400, maxBytes: 32768 },
    verification: singleIdVerification(),
    book: true,
    qmd: { collection: "lapis-server", configPath: ".qmd/index.yml" },
    storybookMirrors: {
      style: "src-spec-mdx",
      directory: "stories/spec",
      titlePrefix: "Specification",
      verifyTarget: true,
      verifyTitle: true,
      verifyContent: true,
      previewPath: ".storybook/preview.ts",
      verifyOrder: true,
    },
    specFirst: {
      mode: "mapped",
      canonicalPattern:
        "^spec/src/current/(?:server|governance|hosted|delivery)\\.md$",
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
