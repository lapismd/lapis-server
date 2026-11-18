import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const plan = {
  tooling: true,
  configure: false,
  rustPackages: [],
  modLoader: false,
  renderer: true,
  imageTooling: false,
};
const jobs = Object.fromEntries(
  [
    "plan",
    "governance",
    "tooling",
    "configure",
    "rust",
    "mod-loader",
    "renderer",
    "image-tooling",
  ].map((name) => [
    name,
    {
      result: ["configure", "rust", "mod-loader", "image-tooling"].includes(
        name,
      )
        ? "skipped"
        : "success",
    },
  ]),
);
const run = (values = jobs) =>
  spawnSync(process.execPath, ["scripts/ci-required.mjs"], {
    env: {
      ...process.env,
      LAPIS_CI_PLAN: JSON.stringify(plan),
      LAPIS_CI_JOBS: JSON.stringify(values),
    },
  });

test("required accepts selected successes and intentional skips", () => {
  assert.equal(run().status, 0);
});

test("required rejects a skipped selected gate", () => {
  assert.notEqual(run({ ...jobs, renderer: { result: "skipped" } }).status, 0);
});
