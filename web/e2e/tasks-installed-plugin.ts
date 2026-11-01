import { rendererPluginImportMap } from "@lapis-notes/app-profile/plugin-host-build";
import { build } from "vite";
import { svelte, vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { cp, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";

/** Test deployment of the unchanged public plugin, through the ordinary loader. */
export async function buildTasksArtifact(repository: string): Promise<string> {
  const directory = join(repository, "test-results/hosted-tasks-artifact");
  const require = createRequire(join(repository, "package.json"));
  const entry = require.resolve("@lapis-notes/lapis-plugin-tasks");
  const pkg = JSON.parse(
    await readFile(join(dirname(dirname(entry)), "package.json"), "utf8"),
  );
  const renderer = JSON.parse(
    await readFile(require.resolve("svelte/package.json"), "utf8"),
  ).version;
  if (pkg.name !== "@lapis-notes/lapis-plugin-tasks")
    throw new Error("Expected the public Tasks package");
  const input = join(repository, ".local/tasks-hosted-entry.mjs");
  await mkdir(dirname(input), { recursive: true });
  await writeFile(
    input,
    'export { default } from "@lapis-notes/lapis-plugin-tasks";\n' +
      'import "@lapis-notes/lapis-plugin-tasks/styles.css";\n',
  );
  const output = await build({
    root: repository,
    configFile: false,
    publicDir: false,
    base: "./",
    logLevel: "warn",
    define: { "process.env.NODE_ENV": JSON.stringify("production") },
    plugins: [
      {
        name: "tasks-acceptance-public-host-modules",
        enforce: "pre",
        resolveId(id) {
          if (id === "svelte" || id.startsWith("svelte/"))
            return {
              id: `svelte/renderer/${renderer}${id.slice("svelte".length)}`,
              external: true,
            };
          if (
            id === "@lapis-notes/api" ||
            id.startsWith("@lapis-notes/api/") ||
            (id.startsWith("@codemirror/") &&
              Object.hasOwn(rendererPluginImportMap.imports, id))
          )
            return { id, external: true };
        },
      },
      svelte({ preprocess: vitePreprocess() }),
    ],
    resolve: { dedupe: ["svelte"] },
    build: {
      lib: {
        entry: input,
        formats: ["es"],
        fileName: () => "main.mjs",
        cssFileName: "styles",
      },
      outDir: directory,
      emptyOutDir: true,
      cssCodeSplit: false,
      minify: false,
      target: "es2022",
      rollupOptions: {
        output: { assetFileNames: "[name][extname]" },
      },
    },
  });
  const outputs = Array.isArray(output) ? output : [output];
  const shared = new Set<string>();
  for (const result of outputs) {
    if (!("output" in result)) throw new Error("Unexpected watch build");
    for (const chunk of result.output)
      if (chunk.type === "chunk")
        for (const specifier of chunk.imports)
          if (specifier.startsWith("@") || specifier.startsWith("svelte/"))
            shared.add(specifier);
  }
  for (const specifier of shared)
    if (!Object.hasOwn(rendererPluginImportMap.imports, specifier))
      throw new Error(`Artifact requires unavailable host module ${specifier}`);
  if (!shared.has("@lapis-notes/api/native-tasks"))
    throw new Error("Tasks artifact does not use the public native port");
  await writeFile(
    join(directory, "manifest.json"),
    JSON.stringify({
      id: "tasks",
      name: "Tasks",
      version: pkg.version,
      minAppVersion: "0.0.1",
      description: pkg.description,
      author: "Lapis Notes",
      lapis: {
        manifestVersion: 1,
        runtime: {
          entries: {
            workspace: {
              path: "main.mjs",
              format: "esm",
              sharedDependencies: [...shared].sort(),
            },
          },
        },
      },
    }),
  );
  return directory;
}

async function files(directory: string, base = directory): Promise<string[]> {
  const result: string[] = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) result.push(...(await files(path, base)));
    else result.push(path.slice(base.length + 1));
  }
  return result.sort();
}

export async function installTasksArtifact(vault: string, artifact: string) {
  const target = join(vault, ".lapis/plugins/tasks");
  await mkdir(target, { recursive: true });
  await cp(artifact, target, { recursive: true });
  const manifest = JSON.parse(
    await readFile(join(target, "manifest.json"), "utf8"),
  );
  const metadata = [];
  for (const path of await files(target)) {
    const bytes = await readFile(join(target, path));
    metadata.push({
      path,
      size: bytes.length,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    });
  }
  const date = new Date().toISOString();
  await writeFile(
    join(vault, ".lapis/installed-plugins.json"),
    JSON.stringify({
      schemaVersion: 1,
      updatedAt: date,
      plugins: {
        tasks: {
          pluginId: "tasks",
          installedVersion: manifest.version,
          installedAt: date,
          updatedAt: date,
          provenance: "development",
          files: metadata,
        },
      },
    }),
  );
  await writeFile(join(vault, ".lapis/community-plugins.json"), '["tasks"]');
  return target;
}
