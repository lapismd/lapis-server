import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { isDeepStrictEqual as equal } from "node:util";
import semver from "semver";
import { verifyWorkspaceNotes } from "./workspace-note.mjs";
import YAML from "yaml";
import {
  verifyCatalogTransition,
  verifyImmutablePackages,
} from "./workspace-catalog.mjs";

import { reviewedTasksTransition } from "./reviewed-tasks-transition.mjs";
import { reviewedNotesPeerTransition } from "./reviewed-notes-peer-transition.mjs";
import { reviewedTerminalTransition } from "./reviewed-terminal-transition.mjs";

const sections = [
  "dependencies",
  "optionalDependencies",
  "devDependencies",
  "peerDependencies",
];
const hash = (value) => createHash("sha256").update(value).digest("hex");
const fail = (reason) => {
  throw Error(`Server dependency intake: ${reason}`);
};
export function intakeFacts(policy, files) {
  const dependencies = [
    "@lapis-notes/api",
    "@lapis-notes/app-profile",
    "@lapis-notes/file-explorer",
    "@lapis-notes/history",
    "@lapis-notes/language-service",
    "@lapis-notes/lapis-plugin-terminal",
    "@lapis-notes/lapis-plugin-tasks",
    "@lapis-notes/markdown",
    "@lapis-notes/search",
    "@lapis-notes/source-editor",
    "@lapis-notes/ui",
    "@lapis-notes/workspace",
    "@lapismd/design-core",
    "@lapismd/jj-history",
    "@lapismd/mira",
    "@lapismd/spec-validator",
  ];
  const expected = (selected) => ({
    schemaVersion: 2,
    kind: "workspace-dependency-intake",
    repository: "lapismd/lapis-server",
    slot: "server-registry",
    dependencies: selected,
    manifests: ["package.json"],
    chapters: [
      "spec/src/current/governance/dependency-automation.md",
      "spec/src/current/governance/dependency-toolchain.md",
    ],
    lockClosure: "registry-transitives",
  });
  assert(
    equal(policy, expected(dependencies)) ||
      equal(
        policy,
        expected(
          dependencies.filter(
            (name) => name !== "@lapis-notes/lapis-plugin-terminal",
          ),
        ),
      ),
    "Unsupported Server dependency intake policy",
  );
  const lock = YAML.parse(files["pnpm-lock.yaml"]),
    workspace = YAML.parse(files["pnpm-workspace.yaml"]);
  const facts = {
    schemaVersion: 2,
    repository: policy.repository,
    manifests: {},
  };
  for (const [path, entry] of Object.entries(lock.importers ?? {}))
    for (const type of sections)
      for (const name of policy.dependencies)
        if (
          entry[type]?.[name] &&
          !policy.manifests.includes(
            path === "." ? "package.json" : `${path}/package.json`,
          )
        )
          fail("undeclared selected importer");
  for (const path of [...policy.manifests].sort()) {
    const manifest = JSON.parse(files[path]),
      importer =
        path === "." || path === "package.json" ? "." : path.slice(0, -13),
      selected = {};
    if (manifest.name !== "@lapismd/lapis-server" || manifest.private !== true)
      fail("private Server owner identity changed");
    for (const type of sections)
      for (const name of [...policy.dependencies].sort()) {
        const declared = manifest[type]?.[name];
        if (declared === undefined) continue;
        const range = declared.startsWith("catalog:")
          ? declared.slice(8)
            ? workspace.catalogs?.[declared.slice(8)]?.[name]
            : workspace.catalog?.[name]
          : declared;
        if (
          !semver.validRange(range) ||
          /^(?:file:|link:|workspace:|https?:|git)/.test(range)
        )
          fail("non-registry selected dependency");
        let version = null,
          integrity = null;
        if (type !== "peerDependencies") {
          const entry = lock.importers?.[importer]?.[type]?.[name];
          version = entry?.version?.split("(")[0];
          integrity =
            lock.packages?.[`${name}@${version}`]?.resolution?.integrity;
          if (
            !semver.valid(version) ||
            semver.prerelease(version) ||
            !semver.satisfies(version, range) ||
            !/^sha512-[A-Za-z0-9+/]{86}==$/.test(integrity ?? "") ||
            !(
              entry.specifier === declared ||
              (declared.startsWith("catalog:") && entry.specifier === range)
            )
          )
            fail("selected range or registry integrity mismatch");
        }
        (selected[type] ??= {})[name] = { declared, range, version, integrity };
      }
    facts.manifests[path] = selected;
  }
  const sdk = "@lapismd/spec-validator";
  const sdkVersion =
    facts.manifests["package.json"].devDependencies?.[sdk]?.version;
  if (
    !/^0\.2\.\d+$/.test(sdkVersion ?? "") ||
    JSON.parse(files["package.json"]).devDependencies[sdk] !== sdkVersion ||
    workspace.overrides?.[sdk] !== sdkVersion ||
    lock.overrides?.[sdk] !== sdkVersion
  )
    fail("validator exact override or supported series mismatch");
  for (const name of policy.dependencies) {
    const versions = new Set(
      Object.values(facts.manifests).flatMap((m) =>
        Object.values(m).flatMap((g) =>
          g[name]?.version ? [g[name].version] : [],
        ),
      ),
    );
    if (versions.size !== 1)
      fail("selected dependency must have one registry identity");
  }
  return facts;
}
export function intakeSlot(policy, facts) {
  const rows = [];
  for (const [path, groups] of Object.entries(facts.manifests))
    for (const [type, entries] of Object.entries(groups))
      for (const [name, fact] of Object.entries(entries))
        rows.push(
          `| \`${path}\` (${type}) | \`${name}\` | \`${fact.range.replaceAll("|", "\\|")}\` | ${fact.version ? `\`${fact.version}\`` : "Peer support"} |`,
        );
  return [
    `<!-- dependency-intake:${policy.slot}:start -->`,
    "",
    "Current registry dependency coordinates; qualification remains separate.",
    "",
    "<!-- prettier-ignore -->",
    "| Manifest | Dependency | Range | Locked version |",
    "| --- | --- | --- | --- |",
    ...rows,
    "",
    `Manifest, peer and integrity facts SHA256: \`${hash(JSON.stringify(facts))}\`.`,
    "",
    `<!-- dependency-intake:${policy.slot}:end -->`,
  ].join("\n");
}
function slot(text, policy) {
  const start = `<!-- dependency-intake:${policy.slot}:start -->`,
    end = `<!-- dependency-intake:${policy.slot}:end -->`;
  if (
    typeof text !== "string" ||
    text.split(start).length !== 2 ||
    text.split(end).length !== 2 ||
    text.indexOf(end) < text.indexOf(start)
  )
    fail("missing or ambiguous managed source slot");
  const a = text.indexOf(start),
    b = text.indexOf(end) + end.length;
  return {
    managed: text.slice(a, b),
    outside: text.slice(0, a) + text.slice(b),
  };
}
export function verifyIntake({
  policyBytes,
  files,
  documents,
  before,
  basePolicyBytes,
  baseDocuments,
  changedFiles = [],
  sourceSha,
}) {
  const policy = JSON.parse(policyBytes),
    facts = intakeFacts(policy, files);
  for (const path of policy.chapters)
    if (slot(documents[path], policy).managed !== intakeSlot(policy, facts))
      fail("managed source facts differ from actual inputs");
  if (!before) return facts;
  const terminal = reviewedTerminalTransition({
    policyBytes,
    files,
    documents,
    before,
    basePolicyBytes,
    baseDocuments,
    changedFiles,
    sourceSha,
  });
  const basePolicy = JSON.parse(basePolicyBytes ?? policyBytes);
  const old = intakeFacts(terminal ? basePolicy : policy, before);
  for (const [path, groups] of Object.entries(old.manifests))
    for (const [type, entries] of Object.entries(groups))
      for (const [name, previous] of Object.entries(entries)) {
        const next = facts.manifests[path][type]?.[name];
        if (!next) fail("selected edge removed");
        if (
          previous.version === next.version &&
          previous.integrity !== next.integrity
        )
          fail("same-version registry integrity changed");
      }
  verifyImmutablePackages(policy, before, files, old, facts);
  if (equal(old, facts)) return facts;
  if (policyBytes !== basePolicyBytes && !terminal)
    fail("missing or changed source opt-in");
  const reviewed =
    terminal ??
    reviewedTasksTransition({
      sourceSha,
      policyBytes,
      basePolicyBytes,
      before,
      files,
      documents,
      baseDocuments,
      changedFiles,
    }) ??
    reviewedNotesPeerTransition({
      sourceSha,
      policyBytes,
      basePolicyBytes,
      before,
      files,
      documents,
      baseDocuments,
      changedFiles,
    });
  const allowed = new Set([
    ...(reviewed ? reviewed.paths : []),
    ...policy.manifests,
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
    ...policy.chapters,
  ]);
  const changesets = changedFiles.filter((p) =>
    /^\.changeset\/lapis-dependencies-[a-f0-9]{12}\.md$/.test(p),
  );
  if (
    changesets.length > 1 ||
    changedFiles.some((p) => !allowed.has(p) && !changesets.includes(p))
  )
    fail("unexpected intake source scope");
  const release = {};
  for (const path of policy.manifests) {
    const a = JSON.parse(before[path]),
      b = JSON.parse(files[path]);
    const strip = (m) => {
      const result = structuredClone(m);
      for (const type of sections)
        for (const name of policy.dependencies)
          if (Object.hasOwn(result[type] ?? {}, name))
            result[type][name] = "<selected>";
      if (reviewed?.added) delete result.dependencies?.[reviewed.added];
      return result;
    };
    if (!equal(strip(a), strip(b)))
      fail("unrelated manifest or public contract changed");
    for (const type of sections)
      for (const name of policy.dependencies) {
        const x = old.manifests[path][type]?.[name],
          y = facts.manifests[path][type]?.[name];
        if (!x && !y) continue;
        if (!x || !y) {
          if (
            reviewed?.added === name &&
            !x &&
            y &&
            path === "package.json" &&
            type === "dependencies"
          )
            continue;
          fail("selected edge added or removed");
        }
        const reviewedEdge =
          reviewed &&
          path === "package.json" &&
          type === "devDependencies" &&
          name === "@lapis-notes/lapis-plugin-tasks" &&
          x.version === "0.1.1" &&
          y.version === "0.2.1" &&
          x.declared === "^0.1.1" &&
          y.declared === "^0.2.1";
        if (
          !reviewedEdge &&
          y.version &&
          (semver.lt(y.version, x.version) ||
            ![null, "patch"].includes(semver.diff(x.version, y.version)))
        )
          fail("only compatible forward patches are supported");
        if (x.declared !== y.declared) {
          if (
            type === "peerDependencies" ||
            x.declared.startsWith("catalog:") !==
              y.declared.startsWith("catalog:") ||
            (!reviewedEdge &&
              !semver.subset(y.range, x.range) &&
              !(
                name === "@lapismd/spec-validator" &&
                type === "devDependencies" &&
                semver.valid(x.range) &&
                semver.valid(y.range) &&
                semver.diff(x.range, y.range) === "patch"
              ))
          )
            fail("unsupported public range or routing change");
          if (!b.private && b.name && type !== "devDependencies")
            release[b.name] = "patch";
        }
      }
  }
  verifyWorkspaceNotes(changesets, files, release);
  verifyCatalogTransition(policy, before, files, old, facts, reviewed);
  for (const path of policy.chapters) {
    const a = slot(baseDocuments?.[path], policy),
      b = slot(documents[path], policy);
    if (
      a.managed !== intakeSlot(terminal ? basePolicy : policy, old) ||
      (!reviewed && a.outside !== b.outside)
    )
      fail("manual canonical prose or stale baseline");
  }
  return facts;
}
