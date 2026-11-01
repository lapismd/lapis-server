import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  testMatch: ["hosted-notes-ui.spec.ts"],
  timeout: 120_000,
  expect: { timeout: 30_000 },
  fullyParallel: false,
  workers: 1,
  reporter: "line",
  outputDir: "../.local/hosted-notes-ui-results",
  preserveOutput: "always",
  use: {
    browserName: "chromium",
    headless: true,
    serviceWorkers: "block",
  },
});
