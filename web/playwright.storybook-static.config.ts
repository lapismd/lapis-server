import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "storybook-static.spec.ts",
  workers: 1,
  timeout: 60_000,
  expect: { timeout: 30_000 },
  outputDir: "../.local/storybook-static-results",
  use: { browserName: "chromium", headless: true, trace: "retain-on-failure" },
});
