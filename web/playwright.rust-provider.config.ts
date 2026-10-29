import { defineConfig } from "@playwright/test";
import hosted from "./playwright.rust-server.config";

export default defineConfig({
  ...hosted,
  testMatch: [
    "rust-server-provider.spec.ts",
    "rust-server-provider-queue.spec.ts",
    "rust-server-provider-resume.spec.ts",
  ],
  timeout: 240_000,
  outputDir: "../../.local/rust-provider-screen-results",
});
