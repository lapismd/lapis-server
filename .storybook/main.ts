import { resolve } from "node:path";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import type { StorybookConfig } from "@storybook/svelte-vite";
import remarkGfm from "remark-gfm";
import { mergeConfig } from "vite";
import {
  rendererExclusions,
  rendererInclusions,
  rendererSingletons,
} from "../web/renderer-dependencies.ts";

const config: StorybookConfig = {
  stories: ["../stories/**/*.mdx", "../stories/**/*.stories.ts"],
  staticDirs: ["../web/public"],
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
    "@storybook/addon-a11y",
    "@storybook/addon-vitest",
  ],
  framework: {
    name: "@storybook/svelte-vite",
    options: { docgen: false },
  },
  async viteFinal(config) {
    const base = process.env.STORYBOOK_BASE_PATH;
    return mergeConfig(config, {
      plugins: [svelte()],
      publicDir: resolve("web/public"),
      build: { target: "esnext" },
      optimizeDeps: {
        exclude: rendererExclusions,
        include: rendererInclusions,
        esbuildOptions: { target: "esnext" },
      },
      resolve: { dedupe: rendererSingletons },
      ...(base ? { base } : {}),
    });
  },
};

export default config;
