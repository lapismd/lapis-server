import { createHash } from "node:crypto";
import { readFile, mkdir, copyFile, chmod, writeFile } from "node:fs/promises";
import { gzipSync } from "node:zlib";
import { resolve, join } from "node:path";

const targets = {
  "aarch64-apple-darwin": "macos-arm64",
  "aarch64-unknown-linux-gnu": "linux-arm64",
  "x86_64-unknown-linux-gnu": "linux-x64",
};
const target = process.argv[2];
const platform = targets[target];
if (!platform) throw Error("Unsupported CLI target");
const pkg = JSON.parse(await readFile("package.json", "utf8"));
const sourceCommit = process.env.GITHUB_SHA;
if (!/^[a-f0-9]{40}$/.test(sourceCommit ?? ""))
  throw Error("Exact source revision required");
const directory = resolve(".local/cli-release");
const staging = join(directory, target);
await mkdir(staging, { recursive: true });
await copyFile(
  `.local/rust-core-target/${target}/release/lapis`,
  join(staging, "lapis"),
);
await chmod(join(staging, "lapis"), 0o755);
await copyFile("crates/lapis-cli/README.md", join(staging, "README.md"));
const file = `lapis-v${pkg.version}-${platform}.tar.gz`;
// Canonical USTAR metadata and gzip headers keep retry bytes identical.
const members = [];
for (const [name, mode] of [
  ["lapis", 0o755],
  ["README.md", 0o644],
]) {
  const contents = await readFile(join(staging, name));
  const header = Buffer.alloc(512);
  const octal = (offset, width, value) =>
    header.write(
      value.toString(8).padStart(width - 1, "0") + "\0",
      offset,
      width,
      "ascii",
    );
  header.write(name, 0, 100, "utf8");
  octal(100, 8, mode);
  octal(108, 8, 0);
  octal(116, 8, 0);
  octal(124, 12, contents.length);
  octal(136, 12, 0);
  header.fill(32, 148, 156);
  header.write("0", 156);
  header.write("ustar\0", 257);
  header.write("00", 263);
  header.write("root", 265);
  header.write("root", 297);
  const checksum = header.reduce((sum, byte) => sum + byte, 0);
  header.write(checksum.toString(8).padStart(6, "0") + "\0 ", 148, 8, "ascii");
  members.push(
    header,
    contents,
    Buffer.alloc((512 - (contents.length % 512)) % 512),
  );
}
members.push(Buffer.alloc(1024));
await writeFile(
  join(directory, file),
  gzipSync(Buffer.concat(members), { level: 9 }),
);
const bytes = await readFile(join(directory, file));
const sha256 = createHash("sha256").update(bytes).digest("hex");
await writeFile(join(directory, `${file}.sha256`), `${sha256}  ${file}\n`);
await writeFile(
  join(directory, `${target}.json`),
  JSON.stringify(
    {
      version: pkg.version,
      sourceCommit,
      target,
      file,
      sha256,
      size: bytes.length,
      label: `Lapis CLI ${pkg.version} · ${platform}`,
      url: `https://github.com/lapismd/lapis-server/releases/download/v${pkg.version}/${file}`,
    },
    null,
    2,
  ) + "\n",
);
