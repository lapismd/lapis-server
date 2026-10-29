import { expect, test } from "@playwright/test";
import { randomBytes } from "node:crypto";
import { mkdir, mkdtemp, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import { manageVaults } from "./tasks-editor-controls";

test("the real Rust binary serves the retained built Web app through login, note persistence and cold restart without Vite", async ({
  page,
}, testInfo) => {
  const repository = resolve("../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  const root = await mkdtemp(join(repository, ".local/rust-static-browser-"));
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const password = randomBytes(32).toString("base64url");
  const service = new RustServerRuntime(
    root,
    repository,
    origin,
    password,
    undefined,
    undefined,
    join(repository, "packages/web/dist-rust-server"),
  );
  const failedAssets: string[] = [];
  page.on("response", (response) => {
    const url = new URL(response.url());
    if (
      url.origin === origin &&
      !url.pathname.startsWith("/api/") &&
      response.status() >= 400
    )
      failedAssets.push(`${response.status()} ${url.pathname}`);
  });
  try {
    await service.start();
    expect(service.processCommands).toEqual([
      join(repository, ".local/rust-core-target/debug/lapis-server"),
    ]);
    const index = await page.goto(`${origin}/notes/example`);
    expect(index?.status()).toBe(200);
    expect(index?.headers()["cross-origin-opener-policy"]).toBe("same-origin");
    expect(index?.headers()["cross-origin-embedder-policy"]).toBe(
      "require-corp",
    );
    await expect(page.locator("html")).toHaveAttribute(
      "data-runtime",
      "rust-server",
    );
    await page.getByLabel("Username").fill("owner");
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await expect(page.getByText("Server DB", { exact: true })).toBeVisible();
    await page.locator('[data-path="Note.md"]').click();
    const editor = page.locator(".cm-content[contenteditable=true]").last();
    await expect(editor).toBeVisible();
    const text =
      "# Saved through the Rust Web server\n\nThe retained UI uses one native origin.\n";
    await editor.fill(text);
    await expect
      .poll(() => readFile(join(service.vault, "Note.md"), "utf8"))
      .toBe(text);
    expect(await page.evaluate(() => window.crossOriginIsolated)).toBe(true);
    expect(
      await page.evaluate(() => navigator.serviceWorker.getRegistrations()),
    ).toEqual([]);
    for (const [path, status] of [
      ["healthz", "ok"],
      ["readyz", "ready"],
    ]) {
      const health = await page.request.get(`${origin}/${path}`);
      expect(health.status()).toBe(200);
      expect(await health.json()).toEqual({ status });
    }
    await manageVaults(page);
    await page.getByRole("button", { name: "Sign out", exact: true }).click();
    await expect(page.getByLabel("Username")).toBeVisible();
    await service.restart();
    expect(service.processCommands).toEqual([
      join(repository, ".local/rust-core-target/debug/lapis-server"),
    ]);
    await page.reload();
    await page.getByLabel("Username").fill("owner");
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await page.locator('[data-path="Note.md"]').click();
    await expect(
      page.locator(".cm-content[contenteditable=true]").last(),
    ).toContainText("Saved through the Rust Web server");
    expect(await readFile(join(service.vault, "Note.md"), "utf8")).toBe(text);
    expect(failedAssets).toEqual([]);
    await manageVaults(page);
    await page.getByRole("button", { name: "Sign out", exact: true }).click();
    await expect(page.getByLabel("Username")).toBeVisible();
  } catch (error) {
    await testInfo.attach("Rust-only Web host log", {
      body: service.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    await service.stop();
  }
});
