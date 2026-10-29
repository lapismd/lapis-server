import { svelte } from "@sveltejs/vite-plugin-svelte";
import type { StorybookConfig } from "@storybook/svelte-vite";
import remarkGfm from "remark-gfm";
import { mergeConfig } from "vite";

const config: StorybookConfig = {
  stories: ["../stories/**/*.mdx", "../stories/**/*.stories.ts"],
  addons: [
    {
      name: "@storybook/addon-docs",
      options: {
        mdxPluginOptions: {
          mdxCompileOptions: {
            remarkPlugins: [remarkGfm],
          },
        },
      },
    },
    "@storybook/addon-themes",
  ],
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
