import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";

const forbidden = "35356e655765143b29a6c9b1518ef3b91d04837d";
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
  if (manifest.includes(forbidden))
    throw new Error("jj-store pull request 22 is not a server dependency");
  if (manifest.includes("github.com/stevejuma/jj-store"))
    throw new Error(
      "jj-store stays a path dependency until pull request 22 merges",
    );
}
if (!core.includes('path = "../../../jj-store/'))
  throw new Error("jj-store path dependency is missing");
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
