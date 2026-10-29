import { createHash } from "node:crypto";
import { appendFileSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { pathToFileURL } from "node:url";

const pins = {
  "darwin-arm64": [
    "aarch64-apple-darwin",
    "51ba42e3d0682616f6eb015045bfe45289b396f03511f9897f645ce8e9272743",
  ],
  "darwin-x64": [
    "x86_64-apple-darwin",
    "6171582d0b5a98a1005cd9643faebff7936812ec264d7968a39d9cef3654a99b",
  ],
  "linux-arm64": [
    "aarch64-unknown-linux-musl",
    "7349a43dd5a20dbc998b10114daa0ee63d2ab863fb822c7eb6b0ebca5903cc69",
  ],
  "linux-x64": [
    "x86_64-unknown-linux-musl",
    "f35438350b5d61963aac5dd74ede510b31d6b9690769d1a6268cf058cc825f72",
  ],
};
export function jjArtifact(platform, arch) {
  const pin = pins[`${platform}-${arch}`];
  if (!pin) throw Error("Unsupported upstream JJ CI platform");
  return { name: `jj-v0.45.1-${pin[0]}.tar.gz`, digest: pin[1] };
}
export function verifyJj(bytes, digest) {
  if (createHash("sha256").update(bytes).digest("hex") !== digest)
    throw Error("Upstream JJ archive checksum mismatch");
}
export function jjArchiveMember(members) {
  const matches = members
    .split("\n")
    .filter((path) => /^(?:\.\/)?jj$/.test(path));
  if (matches.length !== 1)
    throw Error("JJ archive must contain one root executable");
  return matches[0];
}
async function install() {
  if (!process.env.RUNNER_TEMP || !process.env.GITHUB_PATH)
    throw Error("CI tool installation requires runner-owned temporary paths");
  const selected = jjArtifact(process.platform, process.arch);
  const response = await fetch(
    `https://github.com/jj-vcs/jj/releases/download/v0.45.1/${selected.name}`,
  );
  if (!response.ok) throw Error(`JJ download failed: HTTP ${response.status}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  verifyJj(bytes, selected.digest);
  const directory = join(process.env.RUNNER_TEMP, "jj-store-ci-tools");
  mkdirSync(directory, { recursive: true });
  const archive = join(directory, selected.name);
  writeFileSync(archive, bytes);
  const member = jjArchiveMember(
    execFileSync("tar", ["-tzf", archive], { encoding: "utf8" }),
  );
  execFileSync("tar", ["-xzf", archive, "-C", directory, member]);
  const executable = join(directory, "jj");
  const version = execFileSync(executable, ["--version"], {
    encoding: "utf8",
  }).trim();
  if (
    !/^jj 0\.45\.1(?:-7c41cdeb16b6b321c64e789a966b6adf723816a5)?$/.test(version)
  )
    throw Error(`Unexpected upstream JJ version: ${version}`);
  appendFileSync(process.env.GITHUB_PATH, directory + "\n");
  console.log(
    `Installed checksum-verified ${version} in the runner tool directory`,
  );
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
)
  await install();
