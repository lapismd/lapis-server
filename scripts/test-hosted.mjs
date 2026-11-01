import { spawnSync } from "node:child_process";

function run(command, args) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    env: process.env,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

// These fixtures own fresh server profiles; real providers retain their opt-in lane.
run("cargo", ["build", "-p", "lapis-server", "--locked"]);
for (const name of [
  "hosted-notes",
  "server",
  "history",
  "metadata",
  "properties",
  "tasks",
  "static",
]) {
  run("pnpm", [
    "exec",
    "playwright",
    "test",
    "-c",
    name === "hosted-notes"
      ? "web/playwright.hosted-notes.config.ts"
      : `web/playwright.rust-${name}.config.ts`,
  ]);
}
