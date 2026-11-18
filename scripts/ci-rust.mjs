import { spawnSync } from "node:child_process";

const packages = JSON.parse(process.env.LAPIS_CI_RUST_PACKAGES ?? "null");
if (
  !Array.isArray(packages) ||
  packages.length === 0 ||
  new Set(packages).size !== packages.length ||
  packages.some((name) => !/^[a-z][a-z0-9-]*$/u.test(name))
)
  throw Error("Invalid affected Rust package selection");
const selectors = packages.flatMap((name) => ["-p", name]);
for (const args of [
  ["clippy", ...selectors, "--all-targets", "--locked", "--", "-D", "warnings"],
  ["test", ...selectors, "--locked"],
]) {
  const result = spawnSync("cargo", args, { stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
