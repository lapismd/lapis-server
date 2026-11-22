import {
  copyFile,
  mkdir,
  readFile,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { build } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";

const root = resolve(import.meta.dirname, "../web/mcp-apps");
const outDir = resolve(root, "../../.local/mcp-apps-dist");
await mkdir(outDir, { recursive: true });

{
  const document = "notes.html";
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

const providersDir = resolve(outDir, "providers");
await rm(providersDir, { force: true, recursive: true });
const packageDir = process.env.LAPIS_MCP_APP_PROVIDER_PACKAGE;
const localPackage = packageDir
  ? JSON.parse(await readFile(resolve(packageDir, "package.json"), "utf8"))
  : undefined;
const manifests = [];
for (const name of ["@lapis-notes/lapis-plugin-tasks"]) {
  let path;
  if (localPackage?.name === name) {
    const entry = localPackage.exports?.["./mcp-apps.json"];
    if (
      typeof entry !== "string" ||
      !entry.startsWith("./dist/") ||
      entry.includes("..")
    ) {
      throw new Error(
        "The staged provider package has no bundled MCP App export",
      );
    }
    path = resolve(packageDir, entry);
  } else {
    try {
      path = fileURLToPath(import.meta.resolve(`${name}/mcp-apps.json`));
    } catch (error) {
      if (error.code !== "ERR_PACKAGE_PATH_NOT_EXPORTED") throw error;
      continue;
    }
  }
  const manifest = JSON.parse(await readFile(path, "utf8"));
  if (
    manifest.version !== 1 ||
    !/^[a-z0-9-]{1,64}$/u.test(manifest.provider) ||
    !Array.isArray(manifest.apps) ||
    manifests.some((previous) => previous.provider === manifest.provider)
  ) {
    throw new Error(`Invalid MCP App manifest from ${name}`);
  }
  const destination = resolve(providersDir, manifest.provider);
  await mkdir(destination, { recursive: true });
  for (const app of manifest.apps) {
    if (!/^[a-z0-9-]+\.html$/u.test(app.file)) {
      throw new Error("Invalid provider MCP App filename");
    }
    const source = resolve(dirname(path), app.file);
    const size = (await stat(source)).size;
    if (!size || size > 8 * 1024 * 1024) {
      throw new Error(`Missing or oversized provider MCP App: ${app.file}`);
    }
    await copyFile(source, resolve(destination, app.file));
  }
  manifests.push(manifest);
}
if (localPackage && !manifests.length) {
  throw new Error("The staged MCP App package is not an installed provider");
}
await writeFile(resolve(outDir, "providers.json"), JSON.stringify(manifests));
