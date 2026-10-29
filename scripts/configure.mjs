import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";

const forbidden = "35356e655765143b29a6c9b1518ef3b91d04837d";
const revision = "7a98bd89d2bc77fb6ec41f91186992abb19ba161";
const git = 'git = "https://github.com/stevejuma/jj-store"';
const server = await readFile("crates/lapis-server/Cargo.toml", "utf8");
const core = await readFile("crates/lapis-app-core/Cargo.toml", "utf8");
if (!server.includes('name = "lapis-server"'))
  throw new Error("lapis-server package is missing");
if (
  !server.includes('git = "https://github.com/lapismd/lapis-ai"') ||
  !server.includes('tag = "v0.2.0"') ||
  !server.includes('version = "0.2.0"')
)
  throw new Error("lapis-ai-service must stay on tag v0.2.0");
for (const manifest of [server, core]) {
  for (const line of manifest.split("\n")) {
    if (/path\s*=\s*"[^"]*\.\.\/\.\.\/\.\./u.test(line))
      throw new Error(
        "Cargo path dependencies must stay inside this repository",
      );
  }
  if (manifest.includes(forbidden))
    throw new Error("jj-store pull request 22 is not a server dependency");
  const pinned = manifest
    .split("\n")
    .filter((line) => /jj-(?:store|history|vault)/u.test(line));
  if (pinned.length === 0) throw new Error("jj-store dependency is missing");
  for (const line of pinned) {
    if (line.includes('path = "../../../jj-store/'))
      throw new Error("jj-store path dependencies are retired");
    if (!line.includes(git) || !line.includes(`rev = "${revision}"`))
      throw new Error("jj-store must use the merged revision");
    if (/tag\s*=/u.test(line)) throw new Error("jj-store has no crate tag");
  }
}
console.log("Server package manifest configures.");
if (process.argv.includes("--metadata")) {
  const child = spawnSync(
    "cargo",
    [
      "metadata",
      "--format-version",
      "1",
      "--no-deps",
      "--manifest-path",
      "Cargo.toml",
    ],
    { stdio: "inherit" },
  );
  process.exit(child.status ?? 1);
}
