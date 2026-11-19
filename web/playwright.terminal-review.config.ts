import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "terminal-storybook-reload.spec.ts",
  workers: 1,
  timeout: 120_000,
  expect: { timeout: 30_000 },
  outputDir: "../.local/terminal-reload-results",
  use: {
    browserName: "chromium",
    headless: true,
    deviceScaleFactor: 2,
    launchOptions: { timeout: 30_000 },
    // Keep the preview in the production desktop layout with the manager sidebar open.
    viewport: { width: 1536, height: 987 },
  },
  webServer: {
    cwd: new URL("..", import.meta.url).pathname,
    command:
      "pnpm exec storybook dev --host 127.0.0.1 --port 6091 --ci --no-open --disable-telemetry",
    url: "http://127.0.0.1:6091/index.json",
    timeout: 120_000,
    reuseExistingServer: false,
  },
});
