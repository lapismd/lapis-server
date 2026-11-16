import { expect, test } from "@playwright/test";
import { randomBytes } from "node:crypto";
import { mkdir, mkdtemp, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import {
  signInHostedOwner,
  signOutHostedWorkspace,
} from "./rust-hosted-session";

test("released Mermaid math renders and restores through the actual Rust workspace", async ({
  page,
}, info) => {
  const repository = resolve(import.meta.dirname, "../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  const root = await mkdtemp(join(repository, ".local/rust-mermaid-browser-"));
  const password = randomBytes(32).toString("base64url");
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const runtime = new RustServerRuntime(root, repository, origin, password);
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const markdown =
    '# Mermaid intake\n\n```mermaid\nflowchart LR\n  A["$$x^2$$"] --> B["Security cohort"]\n```\n';
  const diagram = page.locator(".markdown-view .mermaid svg").first();
  const expectDiagram = async () => {
    await expect(diagram).toBeVisible();
    await expect(
      diagram.getByText("Security cohort", { exact: true }),
    ).toBeVisible();
    await expect(diagram.locator("math").first()).toBeVisible();
    expect(errors).toEqual([]);
  };
  try {
    await runtime.start();
    await page.goto(origin);
    await signInHostedOwner(page, password);
    await page
      .getByRole("button", {
        name: "Open Rust vault owner · server-hosted",
        exact: true,
      })
      .click();
    await expect(
      page.locator('[data-ui-component="lapis-workspace-shell"]'),
    ).toBeVisible();
    await page.locator('button[data-path="Note.md"]').click();
    const editor = page.locator('.cm-content[contenteditable="true"]').last();
    await expect(editor).toBeVisible();
    await editor.fill(markdown);
    await expect
      .poll(() => readFile(join(runtime.vault, "Note.md"), "utf8"))
      .toBe(markdown);
    await page.getByRole("button", { name: /^Current view: editing/ }).click();
    await expectDiagram();
    await signOutHostedWorkspace(page);
    await runtime.restart();
    await page.reload();
    await signInHostedOwner(page, password);
    await page
      .getByRole("button", {
        name: "Open Rust vault owner · server-hosted",
        exact: true,
      })
      .click();
    await page.locator('button[data-path="Note.md"]').click();
    await expectDiagram();
    expect(await readFile(join(runtime.vault, "Note.md"), "utf8")).toBe(
      markdown,
    );
  } catch (error) {
    await info.attach("Rust Mermaid host log", {
      body: runtime.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    try {
      await signOutHostedWorkspace(page);
    } finally {
      await runtime.stop();
    }
  }
});
