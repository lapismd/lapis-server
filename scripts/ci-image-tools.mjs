import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { jjArchiveMember, jjArtifact, verifyJj } from "./ci-setup-jj.mjs";

const nextestPins = {
  x64: "d039b43c3e0958441b6ad0a39c8884d018b3d2e8fb9928f209074b1100b5e17a",
  arm64: "ca1f81a5b86533eb45ab97e0064aa88e925e53aab9a698ca6331a7cad1f85f07",
};

async function download(url) {
  const response = await fetch(url);
  if (!response.ok)
    throw Error(`Tool download failed: HTTP ${response.status}`);
  return Buffer.from(await response.arrayBuffer());
}
function verify(bytes, digest, name) {
  if (createHash("sha256").update(bytes).digest("hex") !== digest)
    throw Error(`${name} archive checksum mismatch`);
}
async function extract(bytes, member, output) {
  const directory = await mkdtemp(join(tmpdir(), "lapis-server-ci-tool-"));
  try {
    const archive = join(directory, "tool.tar.gz");
    await writeFile(archive, bytes);
    execFileSync("tar", ["-xzf", archive, "-C", directory, member]);
    await mkdir(dirname(output), { recursive: true });
    await writeFile(output, await readFile(join(directory, member)));
    await chmod(output, 0o755);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}
async function install(manifest) {
  if (!nextestPins[process.arch])
    throw Error("Unsupported CI image architecture");
  const jj = jjArtifact("linux", process.arch);
  const jjBytes = await download(
    `https://github.com/jj-vcs/jj/releases/download/v${manifest.versions.jj}/${jj.name}`,
  );
  verifyJj(jjBytes, jj.digest);
  const jjDirectory = await mkdtemp(join(tmpdir(), "lapis-server-ci-jj-"));
  try {
    const archive = join(jjDirectory, jj.name);
    await writeFile(archive, jjBytes);
    const member = jjArchiveMember(
      execFileSync("tar", ["-tzf", archive], { encoding: "utf8" }),
    );
    await extract(jjBytes, member, "/usr/local/bin/jj");
  } finally {
    await rm(jjDirectory, { recursive: true, force: true });
  }
  const target = {
    x64: "x86_64-unknown-linux-gnu",
    arm64: "aarch64-unknown-linux-gnu",
  }[process.arch];
  const nextestName = `cargo-nextest-${manifest.versions.nextest}-${target}.tar.gz`;
  const nextestBytes = await download(
    `https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-${manifest.versions.nextest}/${nextestName}`,
  );
  verify(nextestBytes, nextestPins[process.arch], "Nextest");
  await extract(
    nextestBytes,
    "cargo-nextest",
    "/usr/local/cargo/bin/cargo-nextest",
  );
}
function command(name, args) {
  return execFileSync(name, args, { encoding: "utf8" }).trim();
}
async function check(manifest) {
  const expected = {
    node: `v${manifest.versions.node}`,
    pnpm: manifest.versions.pnpm,
    playwright: `Version ${manifest.versions.playwright}`,
    mdbook: `mdbook v${manifest.versions.mdbook}`,
    jj: `jj ${manifest.versions.jj}`,
    rustc: `rustc ${manifest.versions.rust}`,
    "cargo-nextest": `cargo-nextest ${manifest.versions.nextest}`,
  };
  for (const [name, version] of Object.entries(expected)) {
    const actual = command(name, ["--version"]);
    if (!actual.startsWith(version))
      throw Error(`${name} version mismatch: ${actual}`);
  }
  const directory = await mkdtemp(join(tmpdir(), "lapis-server-ci-jj-check-"));
  try {
    command("git", ["init", directory]);
    execFileSync("jj", ["git", "init", "--colocate", "."], {
      cwd: directory,
      env: { ...process.env, HOME: directory },
      stdio: "pipe",
    });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
  console.log(
    JSON.stringify({ image: manifest.image, tag: manifest.tag, expected }),
  );
}

async function main() {
  const [operation, manifestPath] = process.argv.slice(2);
  if (!["install", "check"].includes(operation) || !manifestPath)
    throw Error("Use ci-image-tools <install|check> <manifest>");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  if (operation === "install") await install(manifest);
  await check(manifest);
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
)
  await main();

export { nextestPins, verify };
