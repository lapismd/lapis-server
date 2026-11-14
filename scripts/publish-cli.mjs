import { readFile, writeFile, readdir, mkdtemp, rm } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { join } from "node:path";
const directory = ".local/cli-release";
const pkg = JSON.parse(await readFile("package.json", "utf8"));
const tag = `v${pkg.version}`;
if (process.env.GITHUB_REF_NAME !== tag) throw Error("Release tag mismatch");
const targets = [
  "aarch64-apple-darwin",
  "aarch64-unknown-linux-gnu",
  "x86_64-unknown-linux-gnu",
];
const archives = await Promise.all(
  targets.map(async (target) => {
    const artifact = JSON.parse(
      await readFile(`${directory}/${target}.json`, "utf8"),
    );
    if (
      artifact.target !== target ||
      artifact.version !== pkg.version ||
      artifact.sourceCommit !== process.env.GITHUB_SHA ||
      !/^lapis-v[\d.]+-(macos-arm64|linux-arm64|linux-x64)\.tar\.gz$/.test(
        artifact.file,
      )
    )
      throw Error("CLI provenance mismatch");
    const bytes = await readFile(`${directory}/${artifact.file}`);
    if (
      createHash("sha256").update(bytes).digest("hex") !== artifact.sha256 ||
      bytes.length !== artifact.size
    )
      throw Error("CLI checksum mismatch");
    return artifact;
  }),
);
await writeFile(
  `${directory}/cli-manifest.json`,
  JSON.stringify(
    {
      schemaVersion: 1,
      version: pkg.version,
      sourceCommit: process.env.GITHUB_SHA,
      archives,
    },
    null,
    2,
  ) + "\n",
);
const existing = JSON.parse(
  execFileSync("gh", ["release", "view", tag, "--json", "assets"], {
    encoding: "utf8",
  }),
).assets;
const temporary = await mkdtemp(join(tmpdir(), "lapis-cli-release-"));
try {
  for (const file of (await readdir(directory)).filter(
    (name) =>
      name.endsWith(".tar.gz") ||
      name.endsWith(".sha256") ||
      name === "cli-manifest.json",
  )) {
    if (existing.some((asset) => asset.name === file)) {
      execFileSync("gh", [
        "release",
        "download",
        tag,
        "--pattern",
        file,
        "--dir",
        temporary,
      ]);
      if (
        !(await readFile(join(temporary, file))).equals(
          await readFile(join(directory, file)),
        )
      )
        throw Error(`Refusing to replace differing release asset: ${file}`);
    } else
      execFileSync("gh", ["release", "upload", tag, join(directory, file)], {
        stdio: "inherit",
      });
  }
} finally {
  await rm(temporary, { recursive: true, force: true });
}
