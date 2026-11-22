import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { build } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";

const root = resolve(import.meta.dirname, "../web/mcp-apps");
const outDir = resolve(root, "../../.local/mcp-apps-dist");
await mkdir(outDir, { recursive: true });

for (const name of ["notes", "tasks"]) {
  const document = `${name}.html`;
  const result = await build({
    configFile: false,
    root,
    base: "./",
    publicDir: false,
    plugins: [svelte(), viteSingleFile()],
    build: {
      outDir,
      write: false,
      target: "es2022",
      modulePreload: false,
      rollupOptions: { input: resolve(root, document) },
    },
  });
  const bundle = Array.isArray(result) ? result[0] : result;
  const html = bundle.output.find(
    (asset) => asset.fileName === document,
  )?.source;
  if (typeof html !== "string" || Buffer.byteLength(html) > 8 * 1024 * 1024) {
    throw new Error(`Missing or oversized MCP App bundle: ${document}`);
  }
  await writeFile(resolve(outDir, document), html);
}
