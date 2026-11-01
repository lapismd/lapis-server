import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import semver from "semver";

const args = process.argv.slice(2);
if (
  !args.length ||
  args.length % 2 ||
  args.some((arg, index) => index % 2 === 0 && arg !== "--manifest")
) {
  throw new Error(
    "Use validation:profile --manifest <archives.json> [--manifest <archives.json>]",
  );
}
const cohort = {};
const sources = [];
for (let index = 1; index < args.length; index += 2) {
  const manifest = resolve(args[index]);
  const entries = JSON.parse(await readFile(manifest, "utf8"));
  const provenance = JSON.parse(
    await readFile(join(dirname(manifest), "provenance.json"), "utf8"),
  );
  if (
    provenance.purpose !== "local-consumer-qualification" ||
    !/^[a-f0-9]{40}$/.test(provenance.revision)
  ) {
    throw new Error("Each archive cohort needs source provenance");
  }
  sources.push({ manifest, revision: provenance.revision });
  for (const entry of entries) {
    if (cohort[entry.name])
      throw new Error(`Duplicate cohort package: ${entry.name}`);
    const archive = resolve(dirname(manifest), entry.path);
    const bytes = await readFile(archive);
    if (createHash("sha256").update(bytes).digest("hex") !== entry.sha256) {
      throw new Error(`Archive integrity mismatch: ${entry.name}`);
    }
    const identity = JSON.parse(
      execFileSync("tar", ["-xOf", archive, "package/package.json"], {
        encoding: "utf8",
      }),
    );
    if (
      identity.name !== entry.name ||
      identity.version !== entry.version ||
      !semver.valid(entry.version)
    ) {
      throw new Error(`Archive identity mismatch: ${entry.name}`);
    }
    const receipt = provenance.archives.find(
      (item) => item.name === entry.name,
    );
    if (receipt?.sha256 !== entry.sha256)
      throw new Error(`Source receipt mismatch: ${entry.name}`);
    const files = execFileSync("tar", ["-tzf", archive], {
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    }).split("\n");
    if (!files.some((file) => file.startsWith("package/dist/")))
      throw new Error(`Built exports missing: ${entry.name}`);
    cohort[entry.name] = { ...entry, archive, revision: provenance.revision };
  }
}
if (!cohort["@lapis-notes/app-profile"])
  throw new Error("The shared Notes application profile archive is missing");
const output = resolve(".local/ui-parity/consumer");
await mkdir(output, { recursive: true });
const require = createRequire(import.meta.url);
const hook = join(output, "pnpmfile.cjs");
await writeFile(
  hook,
  `const semver = require(${JSON.stringify(require.resolve("semver"))});
const cohort = ${JSON.stringify(cohort)};
module.exports = { hooks: { readPackage(manifest) {
  for (const field of ["dependencies", "optionalDependencies", ...(manifest.name === "@lapismd/lapis-server" ? ["devDependencies"] : [])]) {
    for (const [name, range] of Object.entries(manifest[field] || {})) {
      const archive = cohort[name];
      if (!archive) continue;
      if (range === "file:" + archive.archive) continue;
      if (!semver.satisfies(archive.version, range)) throw Error(name + " archive does not satisfy " + range);
      manifest[field][name] = "file:" + archive.archive;
    }
  }
  return manifest;
} } };
`,
);
await writeFile(
  join(output, "source.json"),
  JSON.stringify(
    { purpose: "unpublished-profile-validation", sources, cohort },
    null,
    2,
  ) + "\n",
);
const result = spawnSync(
  "pnpm",
  [
    "install",
    "--no-frozen-lockfile",
    `--config.pnpmfile=${hook}`,
    "--lockfile-dir",
    output,
  ],
  { stdio: "inherit" },
);
if (result.status !== 0) process.exit(result.status ?? 1);
console.log(
  `Installed ${Object.keys(cohort).length} verified public archives from ${sources.length} source cohorts; registry lockfile is unchanged.`,
);
