import path from "node:path";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

const notes = path.resolve(import.meta.dirname, "../vendor/lapis-notes");
const api = process.env.LAPIS_RUST_SERVER_URL ?? "http://127.0.0.1:4810";
const proxy = {
  "/api/v3": { target: api, changeOrigin: false },
  "/__lapis/plugins": { target: api, changeOrigin: false },
};

export default defineConfig({
  root: import.meta.dirname,
  build: { target: "esnext" },
  plugins: [svelte()],
  server: {
    proxy,
    fs: { allow: [path.resolve(import.meta.dirname, ".."), notes] },
  },
  preview: { proxy },
});
