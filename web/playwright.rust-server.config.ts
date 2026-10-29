import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  testMatch: [
    "rust-server-host.spec.ts",
    "rust-server-files.spec.ts",
    "rust-server-chat.spec.ts",
    "rust-server-queue.spec.ts",
    "rust-server-participants.spec.ts",
  ],
  timeout: 120_000,
  expect: { timeout: 30_000 },
  fullyParallel: false,
  workers: 1,
  reporter: "line",
  outputDir: "../../.local/rust-hosted-browser-results",
  preserveOutput: "always",
  use: {
    browserName: "chromium",
    headless: true,
    serviceWorkers: "block",
    trace: "retain-on-failure",
  },
});
