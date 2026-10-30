import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [svelte({ hot: false })],
  resolve: { conditions: ["browser"] },
  ssr: { noExternal: ["lucide-static"] },
  test: {
    environment: "jsdom",
    include: [
      "web/src/hosted-notes-ui.test.ts",
      ".storybook/requirement-toc.test.ts",
    ],
    setupFiles: ["./vitest.setup.ts"],
  },
});
