import { svelte } from "@sveltejs/vite-plugin-svelte";
import type { StorybookConfig } from "@storybook/svelte-vite";
import { mergeConfig } from "vite";

const config: StorybookConfig = {
  stories: ["../stories/**/*.stories.ts"],
  framework: {
    name: "@storybook/svelte-vite",
    options: { docgen: false },
  },
  async viteFinal(config) {
    const base = process.env.STORYBOOK_BASE_PATH;
    return mergeConfig(config, {
      plugins: [svelte()],
      ...(base ? { base } : {}),
    });
  },
};

export default config;
