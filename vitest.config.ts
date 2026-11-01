import { svelte } from "@sveltejs/vite-plugin-svelte";
import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    projects: [
      {
        plugins: [svelte({ hot: false })],
        resolve: { conditions: ["browser"] },
        ssr: { noExternal: ["lucide-static"] },
        test: {
          name: "unit",
          environment: "jsdom",
          include: ["web/src/**/*.test.ts", ".storybook/**/*.test.ts"],
          setupFiles: ["./vitest.setup.ts"],
        },
      },
      {
        plugins: [storybookTest({ configDir: ".storybook" })],
        test: {
          name: "storybook",
          setupFiles: ["./.storybook/vitest.setup.ts"],
          fileParallelism: false,
          testTimeout: 60_000,
          browser: {
            enabled: true,
            headless: true,
            provider: playwright(),
            instances: [
              {
                browser: "chromium",
                name: "desktop",
                viewport: { width: 1280, height: 800 },
              },
              {
                browser: "chromium",
                name: "narrow",
                viewport: { width: 390, height: 844 },
              },
            ],
          },
        },
      },
    ],
  },
});
