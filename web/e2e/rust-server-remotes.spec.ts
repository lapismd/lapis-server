import { test, expect, type Page } from "@playwright/test";
import { mkdtemp, mkdir, writeFile, readFile, readdir } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { randomBytes } from "node:crypto";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import { signInHostedOwner } from "./rust-hosted-session";
import { manageVaults } from "./tasks-editor-controls";

async function settings(page: Page) {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Settings", exact: true });
  await dialog
    .getByRole("button", { name: "Repository History", exact: true })
    .click();
  await expect(
    dialog.getByRole("heading", {
      name: "Remotes and credentials",
      exact: true,
    }),
  ).toBeVisible();
  return dialog;
}

test("cold hosted remotes preserve manual connections and encrypted credentials across detach and restart", async ({
  page,
}, testInfo) => {
  const repository = resolve(import.meta.dirname, "../..");
  const root = await mkdtemp(join(tmpdir(), "lapis-hosted-remotes-"));
  const password = randomBytes(32).toString("base64url");
  const token = randomBytes(32).toString("base64url");
  const key = join(root, "repository-key");
  await writeFile(key, randomBytes(32), { mode: 0o600 });
  const previousKey = process.env.LAPIS_SERVER_REPOSITORY_KEY_FILE;
  process.env.LAPIS_SERVER_REPOSITORY_KEY_FILE = key;
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const runtime = new RustServerRuntime(root, repository, origin, password);
  await mkdir(runtime.vault, { recursive: true });
  await writeFile(join(runtime.vault, "Note.md"), "# Original fixture\n");
  const jj = (args: string[]) =>
    execFileSync("jj", args, {
      cwd: runtime.vault,
      env: { ...process.env, JJ_CONFIG: "/dev/null" },
      stdio: "pipe",
    });
  jj(["git", "init", "--colocate"]);
  jj([
    "--config",
    'user.name="Fixture"',
    "--config",
    'user.email="fixture@example.invalid"',
    "describe",
    "-m",
    "Original fixture",
  ]);
  jj(["bookmark", "create", "publish"]);
  const commands: string[] = [];
  page.on("request", (request) => {
    if (
      request.method() === "POST" &&
      new URL(request.url()).pathname.endsWith("/repository-remotes")
    ) {
      const body = request.postDataJSON();
      if (body.kind) commands.push(body.kind);
    }
  });
  const open = async () => {
    await page.goto(origin);
    const ready = page.locator('[data-notes-session-ready="true"]');
    await expect(
      page.getByRole("heading", { name: /^(Sign in|Your vaults)$/u }).or(ready),
    ).toBeVisible();
    if (await ready.isVisible()) return;
    if (await page.getByLabel("Username").isVisible())
      await signInHostedOwner(page, password);
    await page
      .getByRole("button", {
        name: "Open Rust vault owner · server-hosted",
        exact: true,
      })
      .click();
    await expect(
      page.locator('[data-notes-session-ready="true"]'),
    ).toBeVisible();
  };
  try {
    await runtime.start();
    await open();
    let dialog = await settings(page);
    await expect(
      dialog.getByRole("region", { name: "Original repository", exact: true }),
    ).toBeVisible();
    let backup = dialog.getByRole("region", {
      name: "Vault backup",
      exact: true,
    });
    await backup
      .getByLabel("Remote URL", { exact: true })
      .fill("https://example.invalid/notes.git");
    await backup
      .getByRole("button", { name: "Save connection", exact: true })
      .click();
    await expect(
      backup.getByRole("button", { name: "Test connection", exact: true }),
    ).toBeEnabled();
    await expect(
      dialog.getByRole("switch", {
        name: "Synchronize the connected backup automatically",
        exact: true,
      }),
    ).not.toBeChecked();
    await dialog
      .getByRole("button", { name: "Add access token", exact: true })
      .click();
    const credential = page.getByRole("dialog", {
      name: "Add remote credential",
      exact: true,
    });
    await credential
      .getByLabel("Credential label", { exact: true })
      .fill("Cold server token");
    await credential
      .getByLabel("Host", { exact: true })
      .fill("example.invalid");
    await credential.getByLabel("Username", { exact: true }).fill("fixture");
    await credential.getByLabel("Access token", { exact: true }).fill(token);
    await credential
      .getByRole("button", { name: "Save credential", exact: true })
      .click();
    await expect(
      dialog.getByText("Cold server token", { exact: true }),
    ).toBeVisible();
    await expect(
      dialog.getByText("Encrypted on server", { exact: true }),
    ).toBeVisible();
    await page.goto("about:blank");
    await runtime.restart();
    await open();
    dialog = await settings(page);
    backup = dialog.getByRole("region", { name: "Vault backup", exact: true });
    await expect(backup.getByLabel("Remote URL", { exact: true })).toHaveValue(
      "https://example.invalid/notes.git",
    );
    await expect(
      dialog.getByText("Cold server token", { exact: true }),
    ).toBeVisible();
    await expect(
      dialog.getByRole("switch", {
        name: "Synchronize the connected backup automatically",
        exact: true,
      }),
    ).not.toBeChecked();
    await dialog
      .getByRole("button", { name: "Forget Cold server token", exact: true })
      .click();
    await expect(
      dialog.getByText("Cold server token", { exact: true }),
    ).toHaveCount(0);
    await dialog
      .getByRole("button", { name: "Close settings", exact: true })
      .click();
    await manageVaults(page);
    expect(
      commands.filter((kind) => /fetch|push|publish/iu.test(kind)),
    ).toEqual([]);
    expect(runtime.logs).not.toContain(token);
    const inspect = async (path: string): Promise<void> => {
      for (const entry of await readdir(path, { withFileTypes: true })) {
        const file = join(path, entry.name);
        if (entry.isDirectory()) await inspect(file);
        else
          expect((await readFile(file)).includes(Buffer.from(token))).toBe(
            false,
          );
      }
    };
    await inspect(join(root, "profile"));
    await page.goto("about:blank");
  } catch (error) {
    await testInfo.attach("Cold hosted remotes log", {
      body: runtime.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    await runtime.stop();
    if (previousKey === undefined)
      delete process.env.LAPIS_SERVER_REPOSITORY_KEY_FILE;
    else process.env.LAPIS_SERVER_REPOSITORY_KEY_FILE = previousKey;
  }
});
