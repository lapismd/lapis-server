import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

export function syncCliVersion(root = process.cwd(), check = false) {
  const version = JSON.parse(readFileSync(join(root, "package.json"))).version;
  if (!/^\d+\.\d+\.\d+$/.test(version))
    throw Error("Invalid CLI release version");
  const paths = ["crates/lapis-cli/Cargo.toml", "Cargo.lock"];
  const updates = [];
  for (const path of paths) {
    const file = join(root, path),
      before = readFileSync(file, "utf8");
    const pattern = /(\[package\]\nname = "lapis-cli"\nversion = ")[^"]+("|$)/g;
    const lockPattern =
      /(\[\[package\]\]\nname = "lapis-cli"\nversion = ")[^"]+("|$)/g;
    const target = path.endsWith(".lock") ? lockPattern : pattern;
    if ([...before.matchAll(target)].length !== 1)
      throw Error("CLI version entry is missing or ambiguous");
    const after = before.replace(
      target,
      (_, prefix, suffix) => prefix + version + suffix,
    );
    if (check && after !== before)
      throw Error(`CLI release version drift: ${path}`);
    updates.push([file, after]);
  }
  if (!check) for (const [file, after] of updates) writeFileSync(file, after);
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
)
  syncCliVersion(process.cwd(), process.argv.includes("--check"));
