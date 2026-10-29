import { defineConfig } from "@playwright/test";
import hosted from "./playwright.rust-server.config";

export default defineConfig(hosted, {
  testMatch: [
    "rust-server-metadata.spec.ts",
    "rust-server-search-model.spec.ts",
  ],
  outputDir: "../../.local/rust-hosted-metadata-results",
});
