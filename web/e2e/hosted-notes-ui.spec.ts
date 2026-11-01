import { expect, test } from "@playwright/test";
import { randomBytes } from "node:crypto";
import { mkdtemp, mkdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import {
  signInHostedOwner,
  signOutHostedWorkspace,
} from "./rust-hosted-session";

const password = randomBytes(32).toString("base64url");

for (const renderer of ["preview", "development"] as const) {
  test(`hosted server starts and loads the Notes workspace (${renderer})`, async ({
    page,
  }, testInfo) => {
    const repository = resolve(import.meta.dirname, "../..");
    await mkdir(join(repository, ".local"), { recursive: true });
    // A fresh profile accepts the password file as the initial owner password.
    const root = await mkdtemp(join(repository, ".local/hosted-notes-ui-"));
    const origin = `http://localhost:${await unusedLoopbackPort()}`;
    const runtime = new RustServerRuntime(
      root,
      repository,
      origin,
      password,
      undefined,
      undefined,
      undefined,
      renderer,
    );
    try {
      await runtime.start();
      expect(runtime.processCommands).toHaveLength(2);
      await page.setViewportSize({ width: 1280, height: 720 });
      await page.emulateMedia({ colorScheme: "dark" });
      await page.goto(origin);
      await expect(
        page.getByRole("heading", { name: "Sign in", exact: true }),
      ).toBeVisible();
      await expect(page.locator("html")).toHaveAttribute(
        "data-ui-theme",
        "lapis",
      );
      await expect(page.locator("html")).toHaveClass(/dark/);
      await expect
        .poll(() =>
          page
            .locator("[data-server-login-logo]")
            .evaluate((image: HTMLImageElement) => image.naturalWidth),
        )
        .toBeGreaterThan(0);
      const login = await page.locator(".server-login-card").boundingBox();
      expect(login).not.toBeNull();
      expect(Math.abs(login!.x + login!.width / 2 - 640)).toBeLessThan(1);
      expect(Math.abs(login!.y + login!.height / 2 - 360)).toBeLessThan(1);
      await testInfo.attach("restored-login", {
        body: await page.screenshot(),
        contentType: "image/png",
      });
      await page.keyboard.press("Tab");
      await expect(page.getByLabel("Username")).toBeFocused();
      const loginInputChrome = await page
        .locator('.server-login .server-field input[data-ui-component="input"]')
        .evaluateAll((inputs) => {
          const root = getComputedStyle(document.documentElement);
          const backgroundProbe = document.createElement("span");
          backgroundProbe.style.backgroundColor = root
            .getPropertyValue("--background")
            .trim();
          document.body.append(backgroundProbe);
          const expectedBackground =
            getComputedStyle(backgroundProbe).backgroundColor;
          backgroundProbe.remove();
          const inputProbe = document.createElement("span");
          inputProbe.style.color = root.getPropertyValue("--input").trim();
          document.body.append(inputProbe);
          const expectedFocusRing = getComputedStyle(inputProbe).color;
          inputProbe.remove();
          const focusRingProbe = document.createElement("span");
          focusRingProbe.style.color = getComputedStyle(inputs[0])
            .getPropertyValue("--ui-input-focus-ring-color")
            .trim();
          document.body.append(focusRingProbe);
          const resolvedFocusRing = getComputedStyle(focusRingProbe).color;
          focusRingProbe.remove();
          return {
            backgrounds: inputs.map(
              (input) => getComputedStyle(input).backgroundColor,
            ),
            expectedBackground,
            expectedFocusRing,
            resolvedFocusRing,
          };
        });
      expect(loginInputChrome.backgrounds).toEqual([
        loginInputChrome.expectedBackground,
        loginInputChrome.expectedBackground,
      ]);
      expect(loginInputChrome.resolvedFocusRing).toBe(
        loginInputChrome.expectedFocusRing,
      );
      await expect(page.locator("html")).toHaveAttribute(
        "data-runtime",
        "rust-server",
      );
      await page.getByLabel("Username").fill("owner");
      await page.getByLabel("Password").fill(password);
      await page.getByRole("button", { name: "Sign in", exact: true }).click();
      await expect(
        page.getByRole("heading", { name: "Your vaults", exact: true }),
      ).toBeVisible();
      await page
        .getByRole("button", {
          name: "Open Rust vault owner · server-hosted",
          exact: true,
        })
        .click();
      await expect(
        page.locator('[data-ui-component="lapis-workspace-shell"]'),
      ).toBeVisible();
      const shell = page.locator('[data-ui-component="lapis-workspace-shell"]');
      await expect
        .poll(async () => (await shell.boundingBox())?.height)
        .toBe(720);
      await page.locator('[data-path="Note.md"]').click();
      const editor = page.locator('.cm-content[contenteditable="true"]').last();
      await expect(editor).toBeVisible();
      const saved =
        "# Hosted parity\n\nPersisted through the shared Notes session.\n";
      await editor.fill(saved);
      await expect
        .poll(() => readFile(join(runtime.vault, "Note.md"), "utf8"))
        .toBe(saved);
      await testInfo.attach("restored-workspace", {
        body: await page.screenshot(),
        contentType: "image/png",
      });
      await page.getByRole("button", { name: "Settings", exact: true }).click();
      await expect(
        page.getByRole("button", { name: "Close settings", exact: true }),
      ).toBeVisible();
      await signOutHostedWorkspace(page);
      await page.reload();
      await signInHostedOwner(page, password);
      await page
        .getByRole("button", {
          name: "Open Rust vault owner · server-hosted",
          exact: true,
        })
        .click();
      await expect(
        page.locator('.cm-content[contenteditable="true"]').last(),
      ).toContainText("Hosted parity");
      await signOutHostedWorkspace(page);
    } finally {
      await runtime.stop();
    }
  });
}
