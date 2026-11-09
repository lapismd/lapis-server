import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { format } from "prettier";

const mode = process.argv[2];
if (!["--check", "--write"].includes(mode) || process.argv.length !== 3)
  throw new Error("Use --check or --write");
const root = fileURLToPath(new URL("../", import.meta.url));
const generated = JSON.parse(
  execFileSync(
    "cargo",
    [
      "run",
      "--quiet",
      "--locked",
      "-p",
      "lapis-app-protocol",
      "--bin",
      "export",
    ],
    { cwd: root, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
  ),
);
const outputs = [
  ["protocol.ts", generated.typescript, "typescript"],
  ["schema.json", JSON.stringify(generated.schema), "json"],
];
const stale = [];
for (const [name, value, parser] of outputs) {
  const path = new URL(
    `../crates/lapis-app-protocol/bindings/${name}`,
    import.meta.url,
  );
  const expected = await format(value, { parser });
  if (mode === "--write") await writeFile(path, expected);
  else if (
    (await readFile(path, "utf8").catch((error) => {
      if (error.code === "ENOENT") return "";
      throw error;
    })) !== expected
  )
    stale.push(name);
}
if (stale.length)
  throw new Error(
    `Stale protocol bindings: ${stale.join(", ")}; run pnpm protocol:generate`,
  );
console.log(
  mode === "--write"
    ? "Generated protocol bindings."
    : "Protocol bindings match Rust.",
);
