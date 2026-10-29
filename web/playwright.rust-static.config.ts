import { defineConfig } from "@playwright/test";
import hosted from "./playwright.rust-server.config";
export default defineConfig({
  ...hosted,
  testMatch: ["rust-server-static.spec.ts"],
  outputDir: "../../.local/rust-static-browser-results",
});
