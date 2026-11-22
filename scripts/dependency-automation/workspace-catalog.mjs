import { isDeepStrictEqual as equal } from "node:util";
import YAML from "yaml";

const firstParty = (name) => /^@(?:lapismd|lapis-notes)\//.test(name);
export function verifyImmutablePackages(policy, before, files, old, facts) {
  const previous = YAML.parse(before["pnpm-lock.yaml"]).packages;
  const current = YAML.parse(files["pnpm-lock.yaml"]).packages;
  for (const path of policy.manifests)
    for (const [type, group] of Object.entries(old.manifests[path]))
      for (const [name, value] of Object.entries(group)) {
        const next = facts.manifests[path][type]?.[name];
        if (!value.version || value.version !== next?.version) continue;
        const key = name + "@" + value.version;
        if (!equal(previous[key], current[key]))
          throw Error("Immutable first-party package metadata changed");
      }
}
const fail = (reason) => {
  throw Error(`Server dependency catalog: ${reason}`);
};
export function verifyCatalogTransition(
  policy,
  before,
  files,
  old,
  facts,
  reviewed,
) {
  const a = YAML.parse(before["pnpm-lock.yaml"]),
    b = YAML.parse(files["pnpm-lock.yaml"]);
  const workspace = (bytes) => {
    const result = YAML.parse(bytes);
    if (result.overrides?.["@lapismd/spec-validator"])
      result.overrides["@lapismd/spec-validator"] = "<selected>";
    return result;
  };
  if (
    !equal(
      workspace(before["pnpm-workspace.yaml"]),
      workspace(files["pnpm-workspace.yaml"]),
    )
  )
    fail("unrelated workspace settings changed");
  function graph(lock, current) {
    const versions = Object.fromEntries(
      policy.dependencies.map((name) => [
        name,
        Object.values(current.manifests).flatMap((m) =>
          Object.values(m).flatMap((g) =>
            g[name]?.version ? [g[name].version] : [],
          ),
        )[0],
      ]),
    );
    const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const text = (s) => {
      for (const [name, version] of Object.entries(versions))
        s = s.replace(
          new RegExp(`(^|\\()${escape(name + "@" + version)}(?=$|[()])`, "g"),
          (_m, prefix) => prefix + name + "@<selected>",
        );
      return s.replace(/\([a-f0-9]{32}\)/g, "(<context>)");
    };
    const walk = (value, name = "") => {
      if (typeof value === "string")
        return text(
          Object.hasOwn(versions, name) &&
            value.split("(")[0] === versions[name]
            ? "<selected>" + value.slice(versions[name].length)
            : value,
        );
      if (Array.isArray(value)) return value.map((v) => walk(v, name));
      if (!value || typeof value !== "object") return value;
      const result = {};
      for (const [key, item] of Object.entries(value)) {
        const identity = text(key);
        if (Object.hasOwn(result, identity)) fail("ambiguous peer context");
        result[identity] = walk(
          item,
          key === "version" || key === "specifier" ? name : key,
        );
      }
      return result;
    };
    const settings = walk(
      Object.fromEntries(
        Object.entries(lock).filter(
          ([key]) => !["packages", "snapshots"].includes(key),
        ),
      ),
    );
    for (const row of Object.values(settings.importers ?? {}))
      for (const type of [
        "dependencies",
        "optionalDependencies",
        "devDependencies",
      ])
        for (const name of policy.dependencies) {
          if (row[type]?.[name]) row[type][name].specifier = "<range>";
          if (reviewed?.added === name) delete row[type]?.[name];
        }
    const packages = {};
    for (const [key, row] of Object.entries(lock.packages ?? {}))
      if (firstParty(key)) {
        const name = key.slice(0, key.lastIndexOf("@"));
        if (reviewed?.added === name) continue;
        if (reviewed?.notesPeerTransition && key === "@lapis-notes/api@0.6.3")
          continue;
        // The exact source-bound transition admits only Tasks' reviewed peer
        // declaration evolution. Identity, integrity, registry shape and all
        // snapshot/direct-edge checks still run independently.
        const normalized = walk(row);
        if (
          reviewed?.paths.includes(
            "scripts/dependency-automation/reviewed-tasks-transition.mjs",
          ) &&
          key === "@lapis-notes/lapis-plugin-tasks@0.2.1"
        ) {
          normalized.peerDependencies = {
            ...normalized.peerDependencies,
            "@lapis-notes/api": "^0.2.3",
            "@lapismd/design-core": "^0.7.4",
          };
        }
        if (reviewed?.notesPeerTransition && name === "@lapis-notes/api")
          normalized.peerDependencies = {
            ...normalized.peerDependencies,
            "@lapis-notes/ui": "<reviewed-notes-ui-peer>",
          };
        if (policy.dependencies.includes(name))
          normalized.resolution = {
            ...normalized.resolution,
            integrity: "<selected>",
          };
        const identity = text(key);
        if (Object.hasOwn(packages, identity))
          fail("ambiguous first-party identity");
        packages[identity] = normalized;
      }
    const peers = (s) =>
      [...text(s).matchAll(/(?:^|\()(@(?:lapismd|lapis-notes)\/[^()]+)/g)]
        .map((m) => m[1])
        .sort();
    const edges = [];
    for (const [key, row] of Object.entries(lock.snapshots ?? {})) {
      const references = ["dependencies", "optionalDependencies"].flatMap(
        (type) =>
          Object.entries(row[type] ?? {})
            .filter(([name]) => firstParty(name))
            .map(([name, version]) => [
              type,
              name,
              reviewed?.notesPeerTransition &&
              name === "@lapis-notes/api" &&
              version.split("(")[0] === "0.6.3"
                ? ("<selected>" + version.slice("0.6.3".length)).replace(
                    /\([a-f0-9]{32}\)/g,
                    "(<context>)",
                  )
                : walk(version, name),
            ]),
      );
      if (!firstParty(key) && !peers(key).length && !references.length)
        continue;
      const primary = key.split("(")[0],
        owner = firstParty(primary)
          ? text(primary)
          : primary.slice(0, primary.lastIndexOf("@"));
      const primaryName = primary.slice(0, primary.lastIndexOf("@"));
      if (reviewed?.added === primaryName) continue;
      if (reviewed?.notesPeerTransition && primary === "@lapis-notes/api@0.6.3")
        continue;
      edges.push(
        JSON.stringify([
          owner,
          peers(key),
          references.sort((x, y) =>
            JSON.stringify(x).localeCompare(JSON.stringify(y)),
          ),
        ]),
      );
    }
    return { settings, packages, edges: [...new Set(edges)].sort() };
  }
  const previous = graph(a, old);
  const current = graph(b, facts);
  if (!equal(previous.settings, current.settings))
    fail("unrelated importer or workspace lock setting changed");
  if (
    !equal(previous.packages, current.packages) &&
    !reviewed?.notesCohortTransition
  )
    fail("unrelated first-party metadata changed");
  if (!equal(previous.edges, current.edges) && !reviewed?.notesCohortTransition)
    fail("unrelated first-party edge changed");
  for (const [key, row] of Object.entries(b.packages ?? {}))
    if (
      !equal(a.packages?.[key], row) &&
      (!/^sha512-[A-Za-z0-9+/]{86}==$/.test(row.resolution?.integrity ?? "") ||
        Object.keys(row.resolution).length !== 1)
    )
      fail("changed package lacks registry-only integrity");
}
