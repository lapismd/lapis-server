import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { ColdHttpsHost } from "./repository-https-host.mjs";

export async function qualifyColdHttpsHost(base, endpoint, environment) {
  const host = new ColdHttpsHost(base, environment);
  const run = (bin, args, cwd = base) =>
    execFileSync(bin, args, {
      cwd,
      env: environment,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
  const jj = (args) =>
    run(
      "jj",
      [
        "--no-pager",
        "--config",
        "user.name='HTTPS fixture'",
        "--config",
        "user.email='fixture@invalid'",
        "--config",
        "signing.behavior='drop'",
        ...args,
      ],
      host.vaults[0],
    );
  const target = (snapshot, name = "original") =>
    snapshot.settings.targets.find((t) => t.target === name);
  const facts = () =>
    JSON.parse(readFileSync(join(base, "facts.json"), "utf8"));
  const refs = () =>
    run("git", [
      "--git-dir",
      join(base, "exports/token.git"),
      "for-each-ref",
      "--format=%(refname) %(objectname)",
    ]);
  const secrets = ["first-synthetic-token", "replacement-synthetic-token"];
  jj(["git", "init", "--no-colocate", "."]);
  writeFileSync(join(host.vaults[0], "Note.md"), "Reviewed original bytes\n");
  jj(["commit", "-m", "Cold HTTPS publication"]);
  jj(["bookmark", "create", "publish", "-r", "@-"]);
  jj(["bookmark", "create", "never-publish", "-r", "@-"]);
  const commit = jj(["log", "-r", "publish", "--no-graph", "-T", "commit_id"]);
  writeFileSync(
    join(host.vaults[0], "Uncaptured.md"),
    "Keep these unsnapshotted bytes\n",
  );
  const checkout = readFileSync(
    join(host.vaults[0], ".jj/working_copy/checkout"),
  );
  const tree = readFileSync(
    join(host.vaults[0], ".jj/working_copy/tree_state"),
  );
  const connection = (credential) => ({
    name: "https-fixture",
    url: `${endpoint}/token.git`,
    branch: "host-original",
    bookmark: "publish",
    authentication: "token",
    credential,
  });
  const saveCredential = (client, token, replace) =>
    host.command(client, "save_credential", {
      input: {
        label: "Disposable HTTPS",
        host: new URL(endpoint).host,
        username: "fixture-user",
        provider: "https",
        token,
        replace,
      },
    });
  function scan(path) {
    for (const entry of readdirSync(path, { withFileTypes: true })) {
      const file = join(path, entry.name);
      if (entry.isDirectory()) scan(file);
      else
        for (const secret of secrets)
          assert.equal(
            readFileSync(file).includes(Buffer.from(secret)),
            false,
            "Plaintext credential in host state",
          );
    }
  }
  try {
    await host.start(host.key, "environment");
    let client = await host.attach();
    const saved = await saveCredential(client, secrets[0]);
    const credential = saved.settings.credentials[0].id;
    assert.equal(JSON.stringify(saved).includes(secrets[0]), false);
    const configured = await host.command(client, "save_connection", {
      target: "original",
      connection: connection(credential),
    });
    assert.equal(configured.settings.automaticSync, false);
    assert.equal(facts().receives, 0);
    assert.equal(target(configured, "backup").connection, null);
    const otherVault = await host.settings(await host.attach(1));
    assert.deepEqual(
      otherVault.settings.targets.map(({ target, connection }) => ({
        target,
        connection,
      })),
      [{ target: "backup", connection: null }],
    );
    await host.command(client, "test_connection", { target: "original" });
    assert.equal(facts().receives, 0);
    const review = target(
      await host.command(client, "review_push", { target: "original" }),
    ).review;
    assert.equal(review.commit, commit);
    const pushed = await host.command(client, "push", {
      target: "original",
      reviewId: review.id,
    });
    assert.equal(target(pushed).publication, undefined);
    assert.equal(refs(), `refs/heads/host-original ${commit}`);
    assert.equal(facts().receives, 1);
    await host.command(client, "fetch", { target: "original" });
    const retired = client;
    const removed = await host.request(
      `/api/v3/clients/${retired.client_id}?vault=${retired.vault_id}&generation=${retired.generation}`,
      undefined,
      "DELETE",
    );
    assert.equal(removed.ok, true);
    assert.equal((await host.request(host.route(retired))).ok, false);
    await host.stop();
    await host.start();
    client = await host.attach();
    const restored = await host.settings(client);
    assert.equal(restored.settings.credentials[0].id, credential);
    assert.equal(target(restored).connection.credential, credential);
    assert.equal(facts().receives, 1);
    writeFileSync(join(base, "active-token"), secrets[1]);
    assert.equal(
      (
        await host.request(host.route(client), {
          kind: "test_connection",
          target: "original",
          expectedRevision: restored.revision,
        })
      ).ok,
      false,
    );
    await saveCredential(client, secrets[1], credential);
    await host.command(client, "test_connection", { target: "original" });
    await host.command(client, "fetch", { target: "original" });
    scan(host.profile);
    const beforeAnonymous = facts();
    await host.command(client, "save_connection", {
      target: "backup",
      connection: {
        ...connection(""),
        url: `${endpoint}/anonymous.git`,
        branch: "main",
        bookmark: "",
        authentication: "anonymous",
      },
    });
    await host.command(client, "test_connection", { target: "backup" });
    assert.equal(facts().anonymousReads > beforeAnonymous.anonymousReads, true);
    assert.equal(facts().leaked, 0);
    assert.equal(facts().receives, 1);
    await host.stop();
    for (const key of [undefined, join(base, "wrong-key")]) {
      if (key) writeFileSync(key, Buffer.alloc(32, 19), { mode: 0o600 });
      await host.start(key ?? null);
      const locked = await host.attach();
      const inventory = await host.settings(locked);
      assert.equal(inventory.settings.storage, "locked");
      assert.deepEqual(inventory.settings.credentials, []);
      assert.equal(
        (
          await host.request(host.route(locked), {
            kind: "fetch",
            target: "original",
            expectedRevision: inventory.revision,
          })
        ).ok,
        false,
      );
      await host.stop();
    }
    await host.start();
    client = await host.attach();
    await host.command(client, "forget_credential", { id: credential });
    assert.equal(
      target(await host.settings(client)).status,
      "credentials-required",
    );
    assert.equal(facts().receives, 1);
    assert.deepEqual(
      readFileSync(join(host.vaults[0], ".jj/working_copy/checkout")),
      checkout,
    );
    assert.deepEqual(
      readFileSync(join(host.vaults[0], ".jj/working_copy/tree_state")),
      tree,
    );
    assert.equal(
      readFileSync(join(host.vaults[0], "Uncaptured.md"), "utf8"),
      "Keep these unsnapshotted bytes\n",
    );
    assert.equal(refs(), `refs/heads/host-original ${commit}`);
    await host.stop();
    scan(host.profile);
    for (const secret of secrets)
      assert.equal(host.logs.includes(secret), false);
  } finally {
    await host.stop();
  }
}
