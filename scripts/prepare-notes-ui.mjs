import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";

const packages = [
  ["api", "dist/index.js", "prepack"],
  ["ui", "dist/index.js", "prepack"],
  ["file-explorer", "dist/index.js", "prepack"],
  ["workspace", "dist/index.js", "prepack"],
  ["app-profile", "dist/native-chat/index.js", "build:native-chat"],
];

for (const [name, marker, script] of packages) {
  const cwd = `vendor/lapis-notes/packages/${name}`;
  if (!existsSync(cwd)) {
    console.error(`Missing Notes checkout at ${cwd}`);
    process.exit(1);
  }
  if (existsSync(`${cwd}/${marker}`)) continue;
  const result = spawnSync("pnpm", ["run", script], {
    cwd,
    stdio: "inherit",
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
