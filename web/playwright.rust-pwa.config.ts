import { defineConfig } from "@playwright/test";
import hosted from "./playwright.rust-server.config";

export default defineConfig({
  ...hosted,
  testMatch: ["rust-server-pwa.spec.ts"],
  outputDir: "../.local/rust-pwa-browser-results",
  use: { ...hosted.use, serviceWorkers: "allow" },
});
