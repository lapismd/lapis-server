// Adapted from the Notes application-ownership audit; hosted sources have no compatibility bridge.
/**
 * Reject first-party application code that reads the legacy ambient `app`.
 *
 * The compatibility module is the only production owner allowed to touch
 * `globalThis.app`. TypeScript's binder is also used to distinguish local
 * `app` parameters/props from unresolved or globally declared bare `app`
 * expressions in TypeScript and Svelte instance scripts.
 */
import { readdirSync, readFileSync } from "node:fs";
import { extname, resolve } from "node:path";
import ts from "typescript";

const ROOTS = ["web/src", "stories", ".storybook"];
const PORTABLE_AGENT_ROOTS: string[] = [];
const FORBIDDEN_AGENT_DEPENDENCY =
  /^(?:@agentclientprotocol\/|@modelcontextprotocol\/|@zed-industries\/agent-client-protocol(?:\/|$)|acpx(?:\/|$))/u;
const COMPATIBILITY_MODULE = resolve(
  "node_modules/@lapis-notes/api/dist/application-compatibility.js",
);
const virtualSources = new Map<string, string>();
const rootNames: string[] = [];

function excluded(path: string): boolean {
  return (
    path.includes("/dist/") ||
    path.includes("/.svelte-kit/") ||
    path.includes("/node_modules/") ||
    path.includes("/e2e/") ||
    /\.(?:test|spec)\.(?:svelte\.)?(?:js|ts)$/u.test(path) ||
    path.endsWith(".d.ts")
  );
}

function collect(directory: string): void {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      if (!excluded(`${path}/`)) collect(path);
      continue;
    }
    if (!entry.isFile() || excluded(path)) continue;
    if (extname(path) === ".ts") {
      rootNames.push(path);
      continue;
    }
    if (extname(path) !== ".svelte") continue;
    const source = readFileSync(path, "utf8");
    const scripts = [
      ...source.matchAll(/<script(?:\s[^>]*)?>([\s\S]*?)<\/script>/gu),
    ];
    if (!scripts.length) continue;
    const virtualPath = `${path}.ts`;
    virtualSources.set(
      virtualPath,
      scripts.map((match) => match[1] ?? "").join("\n"),
    );
    rootNames.push(virtualPath);
  }
}

for (const root of ROOTS) collect(resolve(root));

const host = ts.createCompilerHost({
  module: ts.ModuleKind.ESNext,
  noResolve: true,
  target: ts.ScriptTarget.ESNext,
});
const originalFileExists = host.fileExists;
const originalReadFile = host.readFile;
const originalGetSourceFile = host.getSourceFile;
host.fileExists = (path) =>
  virtualSources.has(path) || originalFileExists(path);
host.readFile = (path) => virtualSources.get(path) ?? originalReadFile(path);
host.getSourceFile = (path, languageVersion, onError, shouldCreateNew) => {
  const virtual = virtualSources.get(path);
  return virtual === undefined
    ? originalGetSourceFile(path, languageVersion, onError, shouldCreateNew)
    : ts.createSourceFile(
        path,
        virtual,
        languageVersion,
        true,
        ts.ScriptKind.TS,
      );
};

const program = ts.createProgram({
  rootNames,
  options: {
    module: ts.ModuleKind.ESNext,
    noResolve: true,
    skipLibCheck: true,
    target: ts.ScriptTarget.ESNext,
  },
  host,
});
const checker = program.getTypeChecker();
const findings: Array<{ file: string; line: number; kind: string }> = [];

function isApplicationExpression(node: ts.Identifier): boolean {
  const parent = node.parent;
  return (
    (ts.isPropertyAccessExpression(parent) && parent.expression === node) ||
    (ts.isElementAccessExpression(parent) && parent.expression === node) ||
    (ts.isCallExpression(parent) && parent.expression === node)
  );
}

function isCompatibilitySymbol(symbol: ts.Symbol | undefined): boolean {
  return Boolean(
    symbol?.declarations?.some(
      (declaration) =>
        resolve(declaration.getSourceFile().fileName) === COMPATIBILITY_MODULE,
    ),
  );
}

for (const sourceFile of program.getSourceFiles()) {
  const file = resolve(
    sourceFile.fileName.replace(/\.svelte\.ts$/u, ".svelte"),
  );
  if (!ROOTS.some((root) => file.startsWith(`${resolve(root)}/`))) continue;
  const source = sourceFile.text;
  if (PORTABLE_AGENT_ROOTS.some((root) => file.startsWith(`${root}/`))) {
    for (const match of source.matchAll(
      /(?:from\s*|import\s*\(|require\s*\()\s*["']([^"']+)["']/gu,
    )) {
      if (!FORBIDDEN_AGENT_DEPENDENCY.test(match[1] ?? "")) continue;
      const line = source.slice(0, match.index).split("\n").length;
      findings.push({ file, line, kind: `agent SDK import ${match[1]}` });
    }
  }
  if (file !== COMPATIBILITY_MODULE) {
    for (const match of source.matchAll(
      /globalThis[^\n;]{0,120}(?:\.\s*|\[\s*["'])app\b/gu,
    )) {
      const line = source.slice(0, match.index).split("\n").length;
      findings.push({ file, line, kind: "globalThis.app" });
    }
  }

  const visit = (node: ts.Node): void => {
    if (
      file !== COMPATIBILITY_MODULE &&
      ts.isIdentifier(node) &&
      node.text === "app" &&
      isApplicationExpression(node)
    ) {
      const symbol = checker.getSymbolAtLocation(node);
      if (!symbol || isCompatibilitySymbol(symbol)) {
        const { line } = sourceFile.getLineAndCharacterOfPosition(
          node.getStart(),
        );
        findings.push({ file, line: line + 1, kind: "ambient app" });
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
}

for (const root of PORTABLE_AGENT_ROOTS) {
  const manifestPath = resolve(root, "package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as Record<
    string,
    Record<string, unknown> | undefined
  >;
  for (const field of [
    "dependencies",
    "devDependencies",
    "optionalDependencies",
    "peerDependencies",
  ]) {
    for (const dependency of Object.keys(manifest[field] ?? {})) {
      if (!FORBIDDEN_AGENT_DEPENDENCY.test(dependency)) continue;
      findings.push({
        file: manifestPath,
        line: 1,
        kind: `agent SDK dependency ${dependency}`,
      });
    }
  }
}

if (findings.length) {
  console.error("Ambient application ownership violations:");
  for (const finding of findings) {
    console.error(
      `- ${finding.file.slice(process.cwd().length + 1)}:${finding.line} ${finding.kind}`,
    );
  }
  process.exitCode = 1;
} else {
  console.log(
    "Application ownership audit passed; globalThis.app is compatibility-only and portable packages contain no agent SDK imports.",
  );
}
