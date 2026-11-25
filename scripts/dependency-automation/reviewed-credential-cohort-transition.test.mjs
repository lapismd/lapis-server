import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { verifyIntake } from "./workspace-intake.mjs";

const fixture = JSON.parse(
  gunzipSync(
    readFileSync(
      new URL(
        "../fixtures/reviewed-credential-cohort-transition.json.gz",
        import.meta.url,
      ),
    ),
  ),
);

test("the exact credential cohort is admitted without altering the registry graph", () => {
  const data = structuredClone(fixture);
  verifyIntake(data);
  assert.deepEqual(data, fixture);
});

for (const [name, mutate] of [
  ["wrong base", (data) => (data.sourceSha = "f".repeat(40))],
  ["different lock", (data) => (data.files["pnpm-lock.yaml"] += "\n")],
  ["extra source path", (data) => data.changedFiles.push("Cargo.toml")],
  [
    "ordinary dependency automation",
    (data) =>
      (data.changedFiles = data.changedFiles.filter(
        (path) => !path.endsWith("reviewed-credential-cohort-transition.mjs"),
      )),
  ],
]) {
  test(`the credential cohort rejects ${name}`, () => {
    const data = structuredClone(fixture);
    mutate(data);
    assert.throws(() => verifyIntake(data));
  });
}
