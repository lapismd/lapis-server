import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import YAML from "yaml";

const workflow = YAML.parse(
  readFileSync(
    new URL("../.github/workflows/release.yml", import.meta.url),
    "utf8",
  ),
);
const version = workflow.jobs.changesets;
const writer = version.steps.find((step) =>
  step.uses?.startsWith("changesets/action@"),
);
const intent = "${{ steps.runtime.outputs.hasChangesets == 'true' }}";

test("source-bound Node qualification and actual intent precede all version PR authority", () => {
  const guard = version.steps.find((step) => step.id === "runtime");
  assert.ok(guard, "Version PR authority requires a source runtime guard");
  assert.equal(
    guard.uses,
    "lapismd/lapis-ci/actions/guard-version-runtime@121359a245a334da31bc138933595a5f00f5ba8b",
  );
  assert.deepEqual(guard.with, {
    "qualification-file":
      "scripts/dependency-automation/versioning-runtime.json",
    "detect-release-intent": "true",
  });
  assert.equal(guard.if, undefined);
  assert.equal(guard["continue-on-error"], undefined);
  const install = version.steps.find(
    (step) => step.run === "pnpm install --frozen-lockfile",
  );
  const app = version.steps.find((step) => step.id === "app");
  assert(version.steps.indexOf(install) < version.steps.indexOf(guard));
  assert(version.steps.indexOf(guard) < version.steps.indexOf(app));
  assert.equal(app.if, intent);
  assert.equal(writer.if, intent);
});

test("a qualified empty plan retains GitHub publication without minting the version PR App", () => {
  const publish = version.steps.find(
    (step) => step.run === "pnpm release:github",
  );
  assert.ok(
    publish,
    "An empty Changesets plan must retain tag/release and image/CLI publication",
  );
  assert.equal(
    publish.if,
    "${{ steps.runtime.outputs.hasChangesets == 'false' }}",
  );
  assert.deepEqual(publish.env, { GH_TOKEN: "${{ github.token }}" });
  assert.equal(publish["continue-on-error"], undefined);
  assert.equal(publish.uses, undefined);
});

test("version PRs fail closed on canonical main with a repository-scoped App", () => {
  assert.equal(
    version.if,
    "github.repository == 'lapismd/lapis-server' && github.ref == 'refs/heads/main'",
  );
  const app = version.steps.find((step) =>
    step.uses?.startsWith("actions/create-github-app-token@"),
  );
  assert.ok(app, "version PR writes require an App token");
  assert.equal(app.id, "app");
  assert.equal(app.if, intent);
  assert.equal(app["continue-on-error"], undefined);
  assert.deepEqual(app.with, {
    "client-id": "${{ vars.LAPIS_DEPENDENCY_APP_CLIENT_ID }}",
    "private-key": "${{ secrets.LAPIS_DEPENDENCY_APP_PRIVATE_KEY }}",
    owner: "${{ github.repository_owner }}",
    repositories: "${{ github.event.repository.name }}",
    "permission-contents": "write",
    "permission-pull-requests": "write",
  });
  assert.ok(version.steps.indexOf(app) < version.steps.indexOf(writer));
  assert.equal(writer.env.GITHUB_TOKEN, "${{ steps.app.outputs.token }}");
  assert.equal(writer.with.version, "pnpm release:version");
  assert.equal(writer.with.commitMode, "github-api");
  assert.equal(writer.with.createGithubReleases, false);
});

test("release publication retains the workflow token and existing tag/image/CLI dispatch", () => {
  const checkout = version.steps.find((step) =>
    step.uses?.startsWith("actions/checkout@"),
  );
  assert.equal(checkout.with["persist-credentials"], false);
  assert.equal(writer.with.publish, "pnpm release:github");
  assert.equal(writer.env.GH_TOKEN, "${{ github.token }}");
  assert.equal(version.permissions.actions, "write");
  assert.equal(version.permissions.contents, "write");
  for (const step of version.steps.filter((step) => step.uses))
    assert.match(step.uses, /@[a-f0-9]{40}$/);
});
