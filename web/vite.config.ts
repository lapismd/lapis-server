import path from "node:path";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import {
  rendererExclusions,
  rendererInclusions,
  rendererSingletons,
} from "./renderer-dependencies";
import {
  pluginHostImportMapPlugin,
  rendererPluginHostInputs,
} from "@lapis-notes/app-profile/plugin-host-build";

const api = process.env.LAPIS_RUST_SERVER_URL ?? "http://127.0.0.1:4810";
const proxy = {
  "/api/v3": { target: api, changeOrigin: false },
  "/api/content/v1": { target: api, changeOrigin: false },
  "/mcp": { target: api, changeOrigin: false },
  "/__lapis/plugins": { target: api, changeOrigin: false },
};

export default defineConfig({
  root: import.meta.dirname,
  build: {
    target: "esnext",
    rollupOptions: {
      preserveEntrySignatures: "strict",
      input: {
        index: path.resolve(import.meta.dirname, "index.html"),
        ...rendererPluginHostInputs,
      },
      output: {
        entryFileNames: (chunk) =>
          chunk.name.startsWith("plugin-host-")
            ? `${chunk.name}.js`
            : "assets/[name]-[hash].js",
      },
    },
  },
  plugins: [pluginHostImportMapPlugin(), svelte()],
  optimizeDeps: {
    exclude: rendererExclusions,
    include: rendererInclusions,
    esbuildOptions: { target: "esnext" },
  },
  resolve: { dedupe: rendererSingletons },
  server: {
    proxy,
    fs: { allow: [path.resolve(import.meta.dirname, "..")] },
  },
  preview: { proxy },
});
