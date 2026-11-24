import { execFileSync } from "node:child_process";
import { appendFileSync, readFileSync } from "node:fs";
import { dirname, posix, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const rootPackages = [
  "lapis-app-core",
  "lapis-app-protocol",
  "lapis-cli",
  "lapis-content-http",
  "lapis-content-mcp",
  "lapis-plugin-jobs",
  "lapis-server",
];

function validPath(file) {
  return (
    typeof file === "string" &&
    !file.startsWith("/") &&
    !file.includes("\\") &&
    !/[\r\n\0]/u.test(file) &&
    file.split("/").every((part) => part && part !== "." && part !== "..")
  );
}

export function affectedPackages(files, metadata, root) {
  if (!metadata?.workspace_members?.length) return null;
  const members = metadata.packages.filter((pkg) =>
    metadata.workspace_members.includes(pkg.id),
  );
  const selected = new Set();
  for (const file of files.filter((entry) => entry.startsWith("crates/"))) {
    const absolute = posix.join(root, file);
    const owner = members
      .filter((pkg) => absolute.startsWith(`${dirname(pkg.manifest_path)}/`))
      .sort((a, b) => b.manifest_path.length - a.manifest_path.length)[0];
    if (!owner) return null;
    selected.add(owner.name);
  }
  let previous;
  do {
    previous = selected.size;
    for (const pkg of members) {
      if (pkg.dependencies.some((dependency) => selected.has(dependency.name)))
        selected.add(pkg.name);
    }
  } while (selected.size !== previous);
  return [...selected].sort();
}

export function impact(files, { full = false, metadata, root = "" } = {}) {
  const plan = {
    schemaVersion: 1,
    full,
    governance: true,
    tooling: false,
    configure: false,
    rustPackages: [],
    modLoader: false,
    renderer: false,
    imageTooling: false,
    reasons: [],
  };
  let all = full || !Array.isArray(files) || files.length === 0;
  const selectRootRust = () => {
    plan.rustPackages = [...rootPackages];
    plan.configure = plan.renderer = plan.imageTooling = true;
  };
  const selectAll = (reason) => {
    all = true;
    if (reason) plan.reasons.push(reason);
  };
  for (const file of Array.isArray(files) ? files : []) {
    if (!validPath(file)) {
      selectAll("Invalid changed path");
      continue;
    }
    if (
      /^(?:README|CHANGELOG|LICENSE)\.md$/u.test(file) ||
      file.startsWith("docs/") ||
      file.startsWith("spec/") ||
      /^\.changeset\/[^/]+\.md$/u.test(file)
    ) {
      plan.reasons.push(`${file}: documentation or release note`);
      continue;
    }
    if (
      file === "package.json" ||
      file === "pnpm-lock.yaml" ||
      file.startsWith("web/") ||
      file.startsWith("stories/") ||
      file.startsWith(".storybook/") ||
      /^(?:svelte|vite|vitest|tsconfig)[^/]*\.(?:js|mjs|ts|json)$/u.test(file)
    ) {
      plan.tooling = plan.renderer = true;
      plan.reasons.push(`${file}: renderer or Node dependency input`);
      continue;
    }
    if (
      file.startsWith("scripts/dependency-automation/") ||
      file.startsWith("scripts/fixtures/")
    ) {
      plan.tooling = true;
      plan.reasons.push(`${file}: dependency automation`);
      continue;
    }
    if (
      file === "scripts/configure.mjs" ||
      file === "scripts/demo.sh" ||
      file === "scripts/demo.test.mjs" ||
      file === "scripts/protocol.mjs" ||
      file === "docker-compose.yml" ||
      file.endsWith("compose.example.yaml")
    ) {
      plan.configure = true;
      plan.reasons.push(`${file}: configuration or protocol input`);
      continue;
    }
    if (
      file.startsWith("crates/lapis-container-mods/") ||
      file.startsWith("docker/mod-loader/") ||
      file === ".github/workflows/container-mods.yml" ||
      file === "scripts/test-container-mods.sh"
    ) {
      plan.modLoader = plan.imageTooling = true;
      plan.reasons.push(`${file}: Mod loader or container input`);
      continue;
    }
    if (
      file === "crates/lapis-server/Dockerfile" ||
      file === "crates/lapis-server/container-entrypoint.sh" ||
      file === "scripts/test-image-slim.sh" ||
      file === "scripts/prepare-bundled-skills.mjs"
    ) {
      plan.imageTooling = true;
      plan.reasons.push(`${file}: runtime image input`);
      continue;
    }
    if (file.startsWith("crates/")) {
      const packages = affectedPackages([file], metadata, root);
      if (!packages?.length) {
        selectAll(`${file}: unresolved Rust package ownership`);
        continue;
      }
      plan.rustPackages = [
        ...new Set([...plan.rustPackages, ...packages]),
      ].sort();
      plan.configure = true;
      if (packages.includes("lapis-server") || packages.includes("lapis-cli")) {
        plan.renderer = plan.imageTooling = true;
      }
      plan.reasons.push(`${file}: ${packages.join(", ")}`);
      continue;
    }
    if (
      file === "Cargo.toml" ||
      file === "Cargo.lock" ||
      file === "rust-toolchain.toml" ||
      file.startsWith(".cargo/") ||
      file === ".github/workflows/ci.yml" ||
      file.startsWith("scripts/ci-")
    ) {
      selectAll(`${file}: shared Rust or CI input`);
      continue;
    }
    if (
      file.startsWith("scripts/") &&
      /(?:hosted|plugin-operations|startup-budget|renderer)/u.test(file)
    ) {
      plan.renderer = true;
      plan.reasons.push(`${file}: hosted acceptance input`);
      continue;
    }
    selectAll(`${file}: unknown ownership`);
  }
  if (all) {
    selectRootRust();
    plan.tooling = plan.modLoader = true;
    plan.full = full;
  }
  if (plan.imageTooling) plan.modLoader = true;
  return plan;
}

export function changedFiles(base, source, cwd = process.cwd()) {
  return execFileSync(
    "git",
    ["diff", "--no-renames", "--name-only", "-z", base, source],
    { cwd, encoding: "utf8" },
  )
    .split("\0")
    .filter(Boolean);
}

function sourceRange(env = process.env) {
  const event = env.GITHUB_EVENT_PATH
    ? JSON.parse(readFileSync(env.GITHUB_EVENT_PATH, "utf8"))
    : {};
  const source = event.pull_request?.head?.sha ?? env.GITHUB_SHA;
  const base = event.pull_request?.base?.sha ?? event.before;
  if (!/^[a-f0-9]{40}$/u.test(source ?? ""))
    throw Error("Missing exact CI source revision");
  return {
    source,
    base: /^[a-f0-9]{40}$/u.test(base ?? "") ? base : null,
  };
}

function main() {
  const { source, base } = sourceRange();
  const metadata = JSON.parse(
    execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps"], {
      encoding: "utf8",
    }),
  );
  const full =
    process.env.GITHUB_EVENT_NAME !== "pull_request" || base === null;
  const files = base ? changedFiles(base, source) : [];
  const plan = impact(files, { full, metadata, root: metadata.workspace_root });
  const outputs = {
    source,
    base: base ?? "",
    full,
    tooling: plan.tooling,
    configure: plan.configure,
    rust: plan.rustPackages.length > 0,
    rust_packages: JSON.stringify(plan.rustPackages),
    mod_loader: plan.modLoader,
    renderer: plan.renderer,
    image_tooling: plan.imageTooling,
    cache_write:
      process.env.GITHUB_REPOSITORY === "lapismd/lapis-server" &&
      process.env.GITHUB_REF === "refs/heads/main",
    plan: JSON.stringify({ source, base, files, ...plan }),
  };
  if (process.env.GITHUB_OUTPUT)
    for (const [key, value] of Object.entries(outputs))
      appendFileSync(process.env.GITHUB_OUTPUT, `${key}=${value}\n`);
  console.log(JSON.stringify({ source, base, files, ...plan }, null, 2));
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
)
  main();
