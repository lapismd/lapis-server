import { defineConfig } from "@playwright/test";
import hosted from "./playwright.rust-server.config";

/** Explicit installed-plugin cohort; ordinary hosted checks need no Tasks pack. */
export default defineConfig(hosted, {
  testMatch: [
    "rust-server-tasks.spec.ts",
    "rust-server-tasks-editors.spec.ts",
    "rust-server-tasks-recovery.spec.ts",
    "rust-server-tasks-leave.spec.ts",
  ],
  outputDir: "../../test-results/rust-hosted-tasks",
});
