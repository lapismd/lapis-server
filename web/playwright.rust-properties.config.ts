import { defineConfig } from "@playwright/test";
import hosted from "./playwright.rust-server.config";

/** Separate retained Properties family; preserve the original 15 Tasks cases. */
export default defineConfig(hosted, {
  testMatch: ["rust-server-properties.spec.ts"],
  outputDir: "../../test-results/rust-hosted-properties",
});
