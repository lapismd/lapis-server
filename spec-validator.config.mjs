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
    verification: singleIdVerification({
      files: ["^verification(?:/.*)?\\.md$"],
    }),
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
        "^spec/src/current/(?:server|sync|content|cli|governance(?:/(?:dependency-automation|dependency-toolchain))?|hosted|delivery|mods)\\.md$",
      protected: [
        "^(?:crates/|scripts/|web/|stories/|\\.storybook/|package\\.json$|\\.github/|Cargo\\.toml$|Dockerfile$)",
      ],
      ignore: ["(^|/)(?:node_modules|target|spec/book)/"],
      rules: [
        {
          pattern:
            "^(?:package\\.json|pnpm-workspace\\.yaml|pnpm-lock\\.yaml|\\.github/dependency-automation/canonical-intake\\.json|scripts/dependency-automation/(?:check-canonical-intake|source-intake-reader|maintenance-note|check-workspace-intake|workspace-intake|workspace-catalog|workspace-note)\\.mjs)$",
          chapters: [
            "spec/src/current/governance/dependency-automation.md",
            "spec/src/current/governance/dependency-toolchain.md",
          ],
        },
        {
          pattern:
            "^crates/(?:lapis-app-(?:core|protocol)/src/sync|lapis-server/src/(?:control|http)/sync)(?:/|\\.rs$)",
          chapters: ["spec/src/current/sync.md", "spec/src/current/server.md"],
        },
        {
          pattern: "^crates/lapis-container-mods/",
          chapters: ["spec/src/current/mods.md"],
        },
        { pattern: "^crates/", chapters: ["spec/src/current/server.md"] },
        { pattern: ".*", chapters: ["spec/src/current/governance.md"] },
      ],
    },
  },
  check: { build: true, first: true },
});
