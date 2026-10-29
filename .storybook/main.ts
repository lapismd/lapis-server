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
    return mergeConfig(config, { plugins: [svelte()] });
  },
};

export default config;
