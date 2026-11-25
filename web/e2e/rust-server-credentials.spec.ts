import { expect, test } from "@playwright/test";
import { createHash, randomBytes } from "node:crypto";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import { signInHostedOwner } from "./rust-hosted-session";

const pluginId = "credential-acceptance";
const credentialEnvironment = [
  "LAPIS_SERVER_CREDENTIAL_KEY",
  "LAPIS_SERVER_CREDENTIAL_KEY_FILE",
  "LAPIS_SERVER_CREDENTIAL_PLUGINS",
] as const;

const moduleSource = `import { Plugin } from "@lapis-notes/api";
export default class CredentialAcceptance extends Plugin {
  async onload() {
    const marker = document.createElement("div");
    marker.dataset.testid = "persisted-credential-result";
    const before = await this.secrets.get("proof");
    if (before === null) {
      await this.secrets.set("proof", "disposable-browser-test-value");
      marker.textContent = "created";
    } else {
      marker.textContent = before === "disposable-browser-test-value" ? "reopened" : "unexpected";
    }
    document.body.append(marker);
  }
}`;

async function seedCredentialPlugin(vault: string) {
  const appConfig = join(vault, ".lapis");
  const directory = join(appConfig, "plugins", pluginId);
  await mkdir(directory, { recursive: true });
  const version = "1.0.0";
  const manifest = {
    id: pluginId,
    name: "Credential Acceptance",
    version,
    minAppVersion: "0.0.0",
    requiredCapabilities: ["plugin:secrets"],
    lapis: {
      manifestVersion: 1,
      extensionKind: ["workspace"],
      runtime: {
        entries: {
          workspace: {
            path: "credential.mjs",
            format: "esm",
            sharedDependencies: ["@lapis-notes/api"],
          },
        },
      },
    },
  };
  const entries = [
    ["credential.mjs", moduleSource],
    ["manifest.json", JSON.stringify(manifest)],
  ];
  for (const [path, bytes] of entries) {
    await writeFile(join(directory, path), bytes);
  }
  const date = new Date().toISOString();
  await writeFile(
    join(appConfig, "installed-plugins.json"),
    JSON.stringify({
      schemaVersion: 1,
      updatedAt: date,
      plugins: {
        [pluginId]: {
          pluginId,
          installedVersion: version,
          installedAt: date,
          updatedAt: date,
          provenance: "development",
          files: entries.map(([path, bytes]) => ({
            path,
            size: Buffer.byteLength(bytes),
            sha256: createHash("sha256").update(bytes).digest("hex"),
          })),
        },
      },
    }),
  );
  await writeFile(
    join(appConfig, "community-plugins.json"),
    JSON.stringify([pluginId]),
  );
}

test("an approved plugin reopens its credential after switching Server key inputs", async ({
  page,
}, testInfo) => {
  const repository = resolve(import.meta.dirname, "../..");
  const root = await mkdtemp(join(tmpdir(), "lapis-credential-browser-"));
  const password = randomBytes(24).toString("base64url");
  const key = randomBytes(32);
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const runtime = new RustServerRuntime(
    root,
    repository,
    origin,
    password,
    undefined,
    undefined,
    join(repository, "web/dist"),
  );
  const previous = credentialEnvironment.map((name) => process.env[name]);
  const responses: Array<{ method: string; status: number }> = [];
  page.on("response", (response) => {
    if (new URL(response.url()).pathname.endsWith("/plugin-credentials")) {
      responses.push({
        method: response.request().method(),
        status: response.status(),
      });
    }
  });
  const open = async () => {
    await page.goto(origin);
    const ready = page.locator('[data-notes-session-ready="true"]');
    await expect(
      page.getByRole("heading", { name: /^(Sign in|Your vaults)$/u }).or(ready),
    ).toBeVisible();
    if (await ready.isVisible()) return;
    if (await page.getByLabel("Username").isVisible()) {
      await signInHostedOwner(page, password);
    }
    await page
      .getByRole("button", {
        name: "Open Rust vault owner · server-hosted",
        exact: true,
      })
      .click();
    await expect(ready).toBeVisible();
  };
  const result = page.getByTestId("persisted-credential-result");
  try {
    await seedCredentialPlugin(runtime.vault);
    process.env.LAPIS_SERVER_CREDENTIAL_PLUGINS = pluginId;
    delete process.env.LAPIS_SERVER_CREDENTIAL_KEY_FILE;
    process.env.LAPIS_SERVER_CREDENTIAL_KEY = key.toString("base64");
    await runtime.start();
    await open();
    await expect(result).toHaveText("created");

    await page.goto("about:blank");
    const keyFile = join(root, "fixture.key");
    await writeFile(keyFile, key, { mode: 0o600 });
    delete process.env.LAPIS_SERVER_CREDENTIAL_KEY;
    process.env.LAPIS_SERVER_CREDENTIAL_KEY_FILE = keyFile;
    await runtime.restart();
    await open();
    await expect(result).toHaveText("reopened");
    expect(
      responses.filter(
        ({ method, status }) => method === "GET" && status === 200,
      ),
    ).toHaveLength(2);
  } catch (error) {
    await testInfo.attach("credential host log", {
      body: runtime.logs,
      contentType: "text/plain",
    });
    await testInfo.attach("credential response statuses", {
      body: JSON.stringify(responses),
      contentType: "application/json",
    });
    throw error;
  } finally {
    for (const [index, name] of credentialEnvironment.entries()) {
      const value = previous[index];
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
    try {
      await page.goto("about:blank");
      await runtime.stop();
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  }
});
