import { defineConfig } from "@playwright/test";
import hosted from "./playwright.rust-server.config";

export default defineConfig(hosted, {
  use: { ...hosted.use, serviceWorkers: "allow" },
  testMatch: ["mcp-apps.spec.ts"],
  outputDir: "../.local/mcp-apps-test-results",
});
