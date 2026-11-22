import { expect, test } from "@playwright/test";
import { randomBytes } from "node:crypto";
import { mkdir, mkdtemp, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import {
  signInHostedOwner,
  signOutHostedWorkspace,
} from "./rust-hosted-session";

test("the hosted PWA keeps a local draft through an outage and reconnects quietly", async ({
  page,
  context,
}, testInfo) => {
  const repository = resolve(import.meta.dirname, "../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  const root = await mkdtemp(join(repository, ".local/rust-pwa-browser-"));
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const service = new RustServerRuntime(
    root,
    repository,
    origin,
    randomBytes(32).toString("base64url"),
    undefined,
    undefined,
    join(repository, "web/dist"),
  );
  let failure: unknown;
  try {
    await service.start();
    await page.emulateMedia({ colorScheme: "dark" });
    await page.goto(origin);
    await expect(page.locator("html")).toHaveAttribute("data-pwa-host", "true");
    await expect(page.locator("html")).toHaveAttribute(
      "data-runtime",
      "rust-server",
    );
    await expect(page.locator("html")).toHaveAttribute(
      "data-pwa-titlebar-hidden",
      "false",
    );
    expect(
      (await (await page.request.get(`${origin}/manifest.webmanifest`)).json())
        .name,
    ).toBe("Lapis Server");
    await page.evaluate(() => navigator.serviceWorker.ready.then(() => true));
    await expect
      .poll(() => page.evaluate(() => !!navigator.serviceWorker.controller))
      .toBe(true);
    await signInHostedOwner(page, service.password);
    await page
      .getByRole("button", {
        name: "Open Rust vault owner · server-hosted",
        exact: true,
      })
      .click();
    await page.locator('[data-path="Note.md"]').click();
    const editor = page.locator(".cm-content[contenteditable=true]").last();
    await expect(editor).toBeVisible();
    const status = page
      .locator('[data-status-bar-item-id="lapis-server:connection"]')
      .getByRole("button");
    await expect(status).toHaveAccessibleName("Server connection: Connected");
    await expect(status).toHaveText("");
    await expect(
      page.locator(
        '[data-workspace-status-bar] [data-side="right"] [data-status-bar-item-id="lapis-server:connection"]',
      ),
    ).toHaveCount(1);
    await status.press("Enter");
    await page
      .getByRole("button", { name: "Keep notes on this device" })
      .click();
    await expect(
      page.getByRole("region", { name: "Server connection details" }),
    ).toContainText("Cached notes");
    await expect(page.getByText("1", { exact: true }).last()).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(status).toBeFocused();
    await editor.focus();
    await context.setOffline(true);
    await expect(status).toHaveAccessibleName("Server connection: Offline");
    await expect(editor).toBeFocused();
    await expect(
      page.getByRole("alert").filter({ hasText: /network|fetch|unreachable/i }),
    ).toHaveCount(0);
    await status.click();
    await page
      .getByRole("button", { name: "Browse cached notes and drafts" })
      .click();
    await page.getByRole("button", { name: "Note.md" }).click();
    const text = "# Draft saved while offline\n";
    await page.getByRole("textbox", { name: "Markdown draft" }).fill(text);
    await expect(page.getByText("Local draft", { exact: true })).toBeVisible();
    const cached = await page.evaluate(async () => {
      const stores = await Promise.all(
        (await caches.keys()).map((key) => caches.open(key)),
      );
      return (await Promise.all(stores.map((store) => store.keys())))
        .flat()
        .map(({ url }) => new URL(url).pathname);
    });
    expect(cached).toContain("/index.html");
    expect(
      cached.some((path) => /^\/(?:api|__lapis|oauth|mcp)(?:\/|$)/.test(path)),
    ).toBe(false);
    await page.reload();
    await expect(page.locator("html")).toHaveClass(/theme-dark/);
    await expect(
      page.getByRole("textbox", { name: "Markdown draft" }),
    ).toHaveValue(text);
    expect(await readFile(join(service.vault, "Note.md"), "utf8")).not.toBe(
      text,
    );
    await context.setOffline(false);
    await expect(
      page.getByRole("button", { name: "Save draft to server" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Save draft to server" }).click();
    await expect
      .poll(() => readFile(join(service.vault, "Note.md"), "utf8"))
      .toBe(text);
    await page.getByRole("button", { name: "Return to workspace" }).click();
    await expect(status).toHaveAccessibleName("Server connection: Connected");
    await expect(editor).toContainText("Draft saved while offline");
    await signOutHostedWorkspace(page);
  } catch (error) {
    failure = error;
    await testInfo.attach("Rust host log", {
      body: service.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    try {
      await context.setOffline(false);
      await service.stop();
    } catch (cleanupError) {
      if (failure)
        throw new AggregateError(
          [failure, cleanupError],
          "PWA journey and cleanup failed",
        );
      throw cleanupError;
    }
  }
});
