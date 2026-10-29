import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const pkg = JSON.parse(readFileSync("package.json", "utf8"));
if (pkg.private !== true || pkg.name !== "@lapismd/lapis-server") {
  throw new Error(
    "The server package stays private and is not published to npm",
  );
}
if (!/^\d+\.\d+\.\d+$/.test(pkg.version)) {
  throw new Error(`Unexpected package version: ${pkg.version}`);
}
const tag = `v${pkg.version}`;
const changelog = readFileSync("CHANGELOG.md", "utf8");
const heading = `## ${pkg.version}`;
const start = changelog.indexOf(heading);
if (start < 0) throw new Error(`CHANGELOG.md has no ${heading} section`);
const rest = changelog.slice(start + heading.length);
const next = rest.search(/\n## /);
const notes = (next < 0 ? rest : rest.slice(0, next)).trim();
if (!notes) throw new Error(`CHANGELOG.md section ${heading} is empty`);

function releaseExists() {
  try {
    execFileSync("gh", ["release", "view", tag], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function tagExists() {
  try {
    execFileSync("git", ["ls-remote", "--exit-code", "--tags", "origin", tag], {
      stdio: "ignore",
    });
    return true;
  } catch {
    return false;
  }
}

const hadTag = tagExists();
if (releaseExists()) {
  console.log(`${tag} already has a GitHub release`);
} else {
  const args = ["release", "create", tag, "--title", tag, "--notes", notes];
  if (!hadTag) args.push("--target", process.env.GITHUB_SHA);
  execFileSync("gh", args, { stdio: "inherit" });
}
// A tag created with the workflow token does not start the image workflow.
if (!hadTag) {
  execFileSync("gh", ["workflow", "run", "release-image.yml", "--ref", tag], {
    stdio: "inherit",
  });
}
