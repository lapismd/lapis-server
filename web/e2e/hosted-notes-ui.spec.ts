import { expect, test } from "@playwright/test";
import { randomBytes } from "node:crypto";
import { mkdtemp, mkdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";

const password = randomBytes(32).toString("base64url");

test("hosted server starts and loads the Notes workspace", async ({ page }) => {
  const repository = resolve(import.meta.dirname, "../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  // A fresh profile accepts the password file as the initial owner password.
  const root = await mkdtemp(join(repository, ".local/hosted-notes-ui-"));
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const runtime = new RustServerRuntime(root, repository, origin, password);
  try {
    await runtime.start();
    await page.goto(origin);
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
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await expect(
      page.locator('[data-ui-component="lapis-workspace-shell"]'),
    ).toBeVisible();
    await expect(page.locator('[data-path="Note.md"]')).toBeVisible();
  } finally {
    await runtime.stop();
  }
});
