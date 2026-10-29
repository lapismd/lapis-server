import { test, expect } from "@playwright/test";
import { mkdtemp, mkdir, readFile } from "node:fs/promises";
import { randomBytes, createHash } from "node:crypto";
import { writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import { unzipSync } from "fflate";
import { verifyInstalledPluginLifecycle } from "./rust-plugin-lifecycle";
import {
  signInHostedOwner,
  signOutHostedWorkspace,
} from "./rust-hosted-session";

let runtime: RustServerRuntime;
let origin: string;
const password = randomBytes(32).toString("base64url");
test.beforeAll(async () => {
  const repository = resolve("../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  const root = await mkdtemp(join(repository, ".local/rust-hosted-browser-"));
  origin = `http://localhost:${await unusedLoopbackPort()}`;
  runtime = new RustServerRuntime(root, repository, origin, password);
  await runtime.start();
});
test.afterAll(async () => {
  await runtime?.stop();
});
test.afterEach(async ({ page }) => {
  await signOutHostedWorkspace(page);
});

test("retained hosted login and Notes workspace read and persist through Rust", async ({
  page,
}, testInfo) => {
  try {
    await page.goto(origin);
    await expect(page.locator("html")).toHaveAttribute(
      "data-runtime",
      "rust-server",
    );
    await expect(
      page.getByRole("heading", { name: "Sign in", exact: true }),
    ).toBeVisible();
    await page.getByLabel("Username").fill("owner");
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await expect(
      page.getByRole("heading", { name: "Your vaults", exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Create New Vault" }),
    ).toBeEnabled();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await expect(page.getByText("Server DB", { exact: true })).toBeVisible();
    await page.locator('[data-path="Note.md"]').click();
    const editor = page.locator(".cm-content[contenteditable=true]").last();
    await expect(editor).toBeVisible();
    const text =
      "# Saved through Rust\n\nA retained application screen writes native bytes.\n";
    await editor.fill(text);
    await expect
      .poll(() => readFile(join(runtime.vault, "Note.md"), "utf8"))
      .toBe(text);
    // Native interactive intents survive a lost browser, even after a save.
    // This persistence journey leaves cleanly; lost-client recovery is separate.
    await signOutHostedWorkspace(page);
    await page.reload();
    await signInHostedOwner(page, password);
    await expect(
      page.getByRole("heading", { name: "Your vaults", exact: true }),
    ).toBeVisible();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await page.locator('[data-path="Note.md"]').click();
    await expect(
      page.locator(".cm-content[contenteditable=true]").last(),
    ).toContainText("Saved through Rust");
    expect(
      await page.evaluate(() => navigator.serviceWorker.getRegistrations()),
    ).toEqual([]);
  } catch (error) {
    await testInfo.attach("Rust host log", {
      body: runtime.logs,
      contentType: "text/plain",
    });
    throw error;
  }
});

test("retained chooser creates, renames, archives and restores a vault across a server restart", async ({
  page,
}, testInfo) => {
  try {
    await page.goto(origin);
    await page.getByLabel("Username").fill("owner");
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await page.getByRole("button", { name: "Create New Vault" }).click();
    const dialog = page.getByRole("dialog", { name: "New Server Vault" });
    await dialog.getByLabel("Name").fill("Created through the chooser");
    const created = page.waitForResponse(
      (response) =>
        response.url().endsWith("/api/v3/vaults") &&
        response.request().method() === "POST",
    );
    await dialog
      .getByRole("button", { name: "Create vault", exact: true })
      .click();
    const result = await created;
    expect(result.status()).toBe(201);
    const { vault_id: id } = await result.json();
    await expect(page.getByText("Server DB", { exact: true })).toBeVisible();
    await page
      .getByRole("button", { name: "Create File", exact: true })
      .click();
    const editor = page.locator(".cm-content[contenteditable=true]").last();
    await expect(editor).toBeVisible();
    const text = "# Persist this before archiving\n";
    await editor.fill(text);
    await page.getByRole("button", { name: /Current workspace:/ }).click();
    await page
      .getByRole("menuitem", { name: "Manage Vaults", exact: true })
      .click();
    await page
      .getByRole("button", {
        name: "Open actions for Created through the chooser",
        exact: true,
      })
      .click();
    await page
      .getByRole("menuitem", { name: "Rename...", exact: true })
      .click();
    const rename = page.getByRole("dialog", {
      name: "Rename vault",
      exact: true,
    });
    await rename.getByLabel("Name").fill("Renamed through Rust");
    await rename.getByRole("button", { name: "Save", exact: true }).click();
    await page
      .getByRole("button", {
        name: "Open actions for Renamed through Rust",
        exact: true,
      })
      .click();
    await page.getByRole("menuitem", { name: "Archive", exact: true }).click();
    await expect(
      page.getByRole("button", {
        name: "Renamed through Rust is archived",
        exact: true,
      }),
    ).toBeDisabled();
    const file = join(
      runtime.root,
      "profile",
      "vaults",
      id,
      "content",
      "Untitled.md",
    );
    expect(await readFile(file, "utf8")).toBe(text);
    await signOutHostedWorkspace(page);
    await runtime.restart();
    await page.reload();
    await signInHostedOwner(page, password);
    await expect(
      page.getByRole("button", {
        name: "Renamed through Rust is archived",
        exact: true,
      }),
    ).toBeDisabled();
    await page
      .getByRole("button", {
        name: "Open actions for Renamed through Rust",
        exact: true,
      })
      .click();
    await page.getByRole("menuitem", { name: "Restore", exact: true }).click();
    await page
      .getByRole("button", { name: "Open Renamed through Rust", exact: true })
      .click();
    await page.locator('[data-path="Untitled.md"]').click();
    await expect(
      page.locator(".cm-content[contenteditable=true]").last(),
    ).toContainText("Persist this before archiving");
    expect(await readFile(file, "utf8")).toBe(text);
  } catch (error) {
    await testInfo.attach("Rust host log", {
      body: runtime.logs,
      contentType: "text/plain",
    });
    throw error;
  }
});

test("retained folder and ZIP controls import and export native content across restart", async ({
  page,
}, testInfo) => {
  try {
    await page.addInitScript(() => {
      const file = new File(["# Browser folder note\n"], "Folder.md", {
        type: "text/markdown",
      });
      Object.defineProperty(window, "showDirectoryPicker", {
        configurable: true,
        value: async () => ({
          kind: "directory",
          name: "Browser folder",
          async *entries() {
            yield [
              file.name,
              { kind: "file", name: file.name, getFile: async () => file },
            ];
          },
        }),
      });
    });
    await page.goto(origin);
    await page.getByLabel("Username").fill("owner");
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "Import Folder", exact: false }),
    ).toBeEnabled();
    const uploaded = page.waitForResponse(
      (response) =>
        response.url().includes("/api/v3/vaults/import?") &&
        response.request().method() === "POST",
    );
    await page.getByLabel("Import ZIP", { exact: true }).setInputFiles({
      name: "Imported archive.zip",
      mimeType: "application/zip",
      buffer: await readFile(
        resolve(
          "../../crates/lapis-app-core/tests/fixtures/archives/notes.zip",
        ),
      ),
    });
    const imported = await uploaded;
    expect(imported.status()).toBe(201);
    const { vault_id: id } = await imported.json();
    await expect(
      page.getByRole("button", { name: "Open Imported archive", exact: true }),
    ).toBeVisible();
    await page
      .getByRole("button", {
        name: "Open actions for Imported archive",
        exact: true,
      })
      .click();
    const download = page.waitForEvent("download");
    await page
      .getByRole("menuitem", { name: "Export ZIP", exact: true })
      .click();
    const exported = await download;
    expect(await exported.failure()).toBeNull();
    const path = await exported.path();
    expect(path).toBeTruthy();
    const files = unzipSync(await readFile(path!));
    expect(Buffer.from(files["notes/café.md"]).toString()).toBe(
      "# Imported café\n",
    );
    expect([...files["image.bin"]]).toEqual([0, 128, 255]);
    expect(Buffer.from(files[".lapis/app.json"]).toString()).toBe(
      '{"appearance":"dark"}',
    );
    await page
      .getByRole("button", { name: "Import Folder", exact: false })
      .click();
    const dialog = page.getByRole("dialog", {
      name: "Import Server Vault",
      exact: true,
    });
    await expect(dialog.getByLabel("Name")).toHaveValue("Browser folder");
    await dialog
      .getByRole("button", { name: "Import vault", exact: true })
      .click();
    await expect(
      page.getByRole("button", { name: "Open Browser folder", exact: true }),
    ).toBeVisible();
    await signOutHostedWorkspace(page);
    await runtime.restart();
    await page.reload();
    await signInHostedOwner(page, password);
    await page
      .getByRole("button", { name: "Open Browser folder", exact: true })
      .click();
    await page.locator('[data-path="Folder.md"]').click();
    await expect(
      page.locator(".cm-content[contenteditable=true]").last(),
    ).toContainText("Browser folder note");
    expect(
      await readFile(
        join(runtime.root, "profile/vaults", id, "content/image.bin"),
      ),
    ).toEqual(Buffer.from([0, 128, 255]));
  } catch (error) {
    await testInfo.attach("Rust host log", {
      body: runtime.logs,
      contentType: "text/plain",
    });
    throw error;
  }
});

test("authenticated installed modules load through the built Rust browser origin", async ({
  page,
}) => {
  await page.goto(origin);
  await page.getByLabel("Username").fill("owner");
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Your vaults", exact: true }),
  ).toBeVisible();
  const bytes = 'export const marker = "native module";';
  const hash = createHash("sha256").update(bytes).digest("hex");
  const fingerprint = createHash("sha256")
    .update(`main.mjs:${Buffer.byteLength(bytes)}:${hash}`)
    .digest("hex");
  await mkdir(join(runtime.vault, ".lapis/plugins/portable"), {
    recursive: true,
  });
  await writeFile(
    join(runtime.vault, ".lapis/plugins/portable/main.mjs"),
    bytes,
  );
  await writeFile(
    join(runtime.vault, ".lapis/installed-plugins.json"),
    JSON.stringify({
      schemaVersion: 1,
      plugins: {
        portable: {
          pluginId: "portable",
          installedVersion: "1.0.0",
          files: [
            { path: "main.mjs", size: Buffer.byteLength(bytes), sha256: hash },
          ],
        },
      },
    }),
  );
  const current = await page.request.get(`${origin}/api/v3/auth/current`, {
    headers: { "x-lapis-protocol": "3" },
  });
  const { csrf_token } = await current.json();
  const headers = {
    "x-lapis-protocol": "3",
    "x-lapis-csrf": csrf_token,
    origin,
  };
  const attached = await page.request.post(
    `${origin}/api/v3/vaults/${runtime.vaultId}/clients`,
    { headers, data: { protocol_version: 3 } },
  );
  expect(attached.status()).toBe(200);
  const { client_id } = await attached.json();
  const url = `${origin}/__lapis/plugins/${client_id}/portable/1.0.0/${fingerprint}/main.mjs`;
  expect(
    await page.evaluate(async (url) => (await import(url)).marker, url),
  ).toBe("native module");
  expect(
    (
      await page.request.delete(`${origin}/api/v3/clients/${client_id}`, {
        headers,
      })
    ).status(),
  ).toBe(204);
  expect((await page.request.get(url)).status()).toBe(403);
});

test("retained Plugins UI installs and restores an official signed plugin on Rust", async ({
  page,
}, testInfo) => {
  const repository = resolve("../..");
  const root = await mkdtemp(join(repository, ".local/rust-plugin-install-"));
  const hostOrigin = `http://localhost:${await unusedLoopbackPort()}`;
  const service = new RustServerRuntime(root, repository, hostOrigin, password);
  await service.start();
  try {
    await page.goto(hostOrigin);
    await page.getByLabel("Username").fill("owner");
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await expect(page.getByText("Server DB", { exact: true })).toBeVisible();
    await page
      .locator('button[data-hint-target-id="ribbon:app:plugins"]')
      .click();
    await expect(
      page.getByRole("heading", {
        name: "Make Lapis Notes yours.",
        exact: true,
      }),
    ).toBeVisible();
    await page
      .getByRole("link", { name: "Plugins", exact: true })
      .filter({
        has: page.getByRole("heading", { name: "Plugins", exact: true }),
      })
      .click();
    await page
      .getByRole("textbox", { name: "Search registry plugins", exact: true })
      .fill("Word Count");
    await page
      .getByRole("link", { name: "Open Word Count", exact: true })
      .click();
    const downloaded = page.waitForResponse(
      (response) =>
        response.url().includes("/plugin-bundles/download?source=") &&
        response.request().method() === "GET",
    );
    await page
      .getByRole("button", { name: "Install in Lapis", exact: true })
      .click();
    expect((await downloaded).status()).toBe(200);
    await expect(
      page.getByRole("button", { name: "Installed", exact: true }),
    ).toBeDisabled();
    await page
      .getByRole("button", { name: "Open settings", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Plugin registry", exact: true })
      .click();
    await page.getByRole("tab", { name: "Installed", exact: true }).click();
    await expect(
      page.getByRole("switch", { name: "Disable Word Count", exact: true }),
    ).toBeChecked();
    const enabled = JSON.parse(
      await readFile(
        join(service.vault, ".lapis/community-plugins.json"),
        "utf8",
      ),
    );
    expect(enabled).toContain("wordcount");
    await page
      .getByRole("button", { name: "Close settings", exact: true })
      .click();
    await signOutHostedWorkspace(page);
    await service.restart();
    await page.reload();
    await signInHostedOwner(page, password);
    const restoredModule = page.waitForResponse(
      (response) =>
        response.url().includes("/__lapis/plugins/") &&
        response.url().includes("/wordcount/") &&
        response.request().method() === "GET",
    );
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await expect(page.getByText("Server DB", { exact: true })).toBeVisible();
    await page
      .getByRole("button", { name: "Open settings", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Plugin registry", exact: true })
      .click();
    await page.getByRole("tab", { name: "Installed", exact: true }).click();
    await expect(
      page.getByRole("switch", { name: "Disable Word Count", exact: true }),
    ).toBeChecked();
    const restored = await restoredModule;
    expect(restored.status()).toBe(200);
    await verifyInstalledPluginLifecycle(page, service.vault, restored.url());
  } catch (error) {
    await testInfo.attach("Rust plugin host log", {
      body: service.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    try {
      await signOutHostedWorkspace(page);
    } finally {
      await service.stop();
    }
  }
});
