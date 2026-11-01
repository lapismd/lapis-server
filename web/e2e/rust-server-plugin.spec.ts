import { test, expect } from "@playwright/test";
import { mkdtemp, readFile } from "node:fs/promises";
import { randomBytes } from "node:crypto";
import { resolve, join } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import { verifyInstalledPluginLifecycle } from "./rust-plugin-lifecycle";
import {
  signInHostedOwner,
  signOutHostedWorkspace,
} from "./rust-hosted-session";

test.skip(
  process.env.LAPIS_SERVER_PLUGIN_INSTALL_TEST !== "1",
  "Temporarily opt-in: the official Word Count release asset returns 404 in hosted CI.",
);

test("retained Plugins UI installs and restores an official signed plugin on Rust", async ({
  page,
}, testInfo) => {
  const repository = resolve(import.meta.dirname, "../..");
  const root = await mkdtemp(join(repository, ".local/rust-plugin-install-"));
  const password = randomBytes(32).toString("base64url");
  const hostOrigin = `http://localhost:${await unusedLoopbackPort()}`;
  const service = new RustServerRuntime(root, repository, hostOrigin, password);
  await service.start();
  try {
    await page.goto(hostOrigin);
    await page.getByLabel("Username").fill("owner");
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await page
      .getByRole("button", {
        name: "Open Rust vault owner · server-hosted",
        exact: true,
      })
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
    await page.getByRole("button", { name: "Settings", exact: true }).click();
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
      .getByRole("button", {
        name: "Open Rust vault owner · server-hosted",
        exact: true,
      })
      .click();
    await expect(page.getByText("Server DB", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Settings", exact: true }).click();
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
