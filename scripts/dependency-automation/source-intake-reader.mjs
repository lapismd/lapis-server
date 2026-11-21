import { existsSync, readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { changesFromVcs } from "@lapismd/spec-validator";
export function sourceIntakeReader(
  root,
  execute = execFileSync,
  hasJj = existsSync(resolve(root, ".jj")),
  env = execute === execFileSync ? process.env : {},
) {
  const run = (binary, args) =>
    execute(binary, args, {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 3 * 1024 * 1024,
    });
  let parent;
  if (env.GITHUB_EVENT_NAME === "pull_request" && env.GITHUB_EVENT_PATH) {
    const event = JSON.parse(readFileSync(env.GITHUB_EVENT_PATH, "utf8"));
    parent = event.pull_request?.base?.sha;
  }
  parent ??= (
    hasJj
      ? run("jj", [
          "--ignore-working-copy",
          "log",
          "-r",
          "@",
          "--no-graph",
          "-T",
          'parents.map(|p| p.commit_id()).join("\\n")',
        ])
      : run("git", ["rev-parse", "HEAD^1"])
  )
    .trim()
    .split("\n")[0];
  if (!/^[a-f0-9]{40}$/.test(parent))
    throw Error("Source intake ancestry unavailable");
  const read = (path) =>
    hasJj
      ? run("jj", [
          "--ignore-working-copy",
          "file",
          "show",
          "-r",
          parent,
          "--",
          path,
        ])
      : run("git", ["show", `${parent}:${path}`]);
  read.sourceSha = parent;
  read.changedFiles = () =>
    changesFromVcs(
      { base: parent, head: hasJj ? "@" : "HEAD" },
      root,
      (binary, args) => run(binary, args),
    ).map((change) => change.path);
  return read;
}
