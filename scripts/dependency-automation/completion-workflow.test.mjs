import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { parse } from "yaml";
const pin = "053ac8be573871ccc5b0232bc91d8ce7f3bff02f";
const workflow = parse(
  readFileSync(
    new URL(
      "../../.github/workflows/dependency-completed.yml",
      import.meta.url,
    ),
    "utf8",
  ),
);
test("completion observes only reviewed Server source workflows", () => {
  assert.deepEqual(workflow.on, {
    workflow_run: {
      workflows: ["Server CI", "Publish Storybook to GitHub Pages", "Release"],
      types: ["completed"],
    },
  });
  assert.equal(workflow.concurrency, undefined);
});
test("completion has only read-only source metadata authority", () => {
  assert.deepEqual(workflow.permissions, {
    contents: "read",
    actions: "read",
    "pull-requests": "read",
  });
  assert.deepEqual(Object.keys(workflow.jobs), ["notify"]);
});
test("the trusted callback loads implementation and policy from the same reviewed pin", () => {
  const job = workflow.jobs.notify;
  assert.equal(job.if, "github.event.workflow_run.conclusion == 'success'");
  assert.equal(
    job.uses,
    "lapismd/dependency-automation/.github/workflows/completion.yml@" + pin,
  );
  assert.deepEqual(job.with, { "automation-ref": pin });
  assert.deepEqual(job.secrets, {
    "app-private-key": "${{ secrets.LAPIS_DEPENDENCY_APP_PRIVATE_KEY }}",
  });
  for (const key of [
    "steps",
    "env",
    "continue-on-error",
    "permissions",
    "strategy",
  ])
    assert.equal(job[key], undefined);
});
test("all Server CI checkouts retain complete history for the independent intake", () => {
  const ci = parse(
    readFileSync(
      new URL("../../.github/workflows/ci.yml", import.meta.url),
      "utf8",
    ),
  );
  const steps = Object.values(ci.jobs).flatMap((job) => job.steps ?? []);
  const checkouts = steps.filter((step) =>
    step.uses?.startsWith("actions/checkout@"),
  );
  assert.ok(checkouts.length >= 2);
  for (const step of checkouts) {
    assert.equal(step.with["fetch-depth"], 0);
    assert.equal(step.with["persist-credentials"], false);
  }
});
