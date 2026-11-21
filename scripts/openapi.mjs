import { execFileSync } from "node:child_process";
import { readdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { format } from "prettier";

const mode = process.argv[2];
if (!["--write", "--check"].includes(mode) || process.argv.length !== 3)
  throw new Error("Use --write or --check");

const root = fileURLToPath(new URL("../", import.meta.url));
const generated = JSON.parse(
  execFileSync(
    "cargo",
    [
      "run",
      "--quiet",
      "--locked",
      "-p",
      "lapis-server",
      "--bin",
      "lapis-openapi",
    ],
    { cwd: root, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
  ),
);

if (generated.openapi !== "3.1.0" || !generated.paths || !generated.components)
  throw new Error("Invalid OpenAPI document");

function validateReferences(value) {
  if (!value || typeof value !== "object") return;
  if (typeof value.$ref === "string" && value.$ref.startsWith("#/")) {
    const resolved = value.$ref
      .slice(2)
      .split("/")
      .map((segment) => segment.replaceAll("~1", "/").replaceAll("~0", "~"))
      .reduce((root, segment) => root?.[segment], generated);
    if (!resolved) throw new Error(`Unresolved OpenAPI ref: ${value.$ref}`);
  }
  for (const child of Object.values(value)) validateReferences(child);
}

for (const [path, item] of Object.entries(generated.paths)) {
  for (const [method, operation] of Object.entries(item)) {
    if (
      !operation.summary ||
      !operation.responses ||
      !Array.isArray(operation.security)
    )
      throw new Error(`Incomplete operation: ${method.toUpperCase()} ${path}`);
    for (const [, parameter] of path.matchAll(/\{([^}]+)\}/gu)) {
      if (
        !operation.parameters?.some(
          (entry) =>
            entry.name === parameter && entry.in === "path" && entry.required,
        )
      )
        throw new Error(`Missing path parameter ${parameter} in ${path}`);
    }
  }
}
validateReferences(generated.paths);
validateReferences(generated.components);

const adapters = [
  "crates/lapis-server/src/http",
  "crates/lapis-content-http/src",
  "crates/lapis-content-mcp/src",
];
for (const dir of adapters) {
  for (const entry of await readdir(new URL(`../${dir}/`, import.meta.url), {
    recursive: true,
  })) {
    if (!entry.endsWith(".rs") || /(?:^|\/)[^/]*tests?(?:\.rs|\/)/u.test(entry))
      continue;
    const source = await readFile(
      new URL(`../${dir}/${entry}`, import.meta.url),
      "utf8",
    );
    for (const [, route] of source.matchAll(/\.route\s*\(\s*"([^"]+)"/gu)) {
      // The disabled content router's refusal paths are implementation fallbacks.
      if (
        entry === "content/mod.rs" &&
        ["/mcp", "/api/content/v1", "/api/content/v1/{*path}"].includes(route)
      )
        continue;
      if (!generated.paths[route.replace("{*", "{")])
        throw new Error(
          `Route is missing from OpenAPI: ${route} (${dir}/${entry})`,
        );
    }
  }
}

const spec = new URL("../docs/openapi.json", import.meta.url);
const canonical = await format(JSON.stringify(generated), { parser: "json" });
if (mode === "--write") await writeFile(spec, canonical);
else if (canonical !== (await readFile(spec, "utf8").catch(() => "")))
  throw new Error("Stale OpenAPI document: run pnpm openapi:generate");
console.log(`OpenAPI covers ${Object.keys(generated.paths).length} paths.`);
