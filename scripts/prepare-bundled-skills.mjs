import { createRequire } from "node:module";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
const require = createRequire(import.meta.url);
const output = resolve(process.argv[2] ?? ".local/bundled-skills.json");
// This host's static Notes profile owns Markdown; other skills arrive in plugin archives.
const bundles = JSON.parse(
  await readFile(require.resolve("@lapis-notes/markdown/skills.json"), "utf8"),
);
await mkdir(dirname(output), { recursive: true });
await writeFile(output, JSON.stringify(bundles) + "\n");
