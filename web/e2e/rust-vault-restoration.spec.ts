import { expect, test } from "@playwright/test";
import { randomBytes } from "node:crypto";
import { mkdir, mkdtemp, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import {
  signInHostedOwner,
  signOutHostedWorkspace,
} from "./rust-hosted-session";
import { manageVaults, showEditingProblems } from "./tasks-editor-controls";

let runtime: RustServerRuntime;
let origin: string;
const password = randomBytes(32).toString("base64url");
test.beforeEach(async () => {
  const repository = resolve(import.meta.dirname, "../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  const root = await mkdtemp(join(repository, ".local/rust-vault-restore-"));
  origin = `http://localhost:${await unusedLoopbackPort()}`;
  runtime = new RustServerRuntime(root, repository, origin, password);
  await runtime.start();
});
test.afterEach(async ({ page }) => {
  try {
    await signOutHostedWorkspace(page);
  } finally {
    await runtime?.stop();
  }
});

test("authenticated reload restores the selected vault and saved note with a fresh client", async ({
  page,
}, info) => {
  try {
    await page.goto(origin);
    await signInHostedOwner(page, password);
    const attached = page.waitForResponse((response) =>
      response.url().endsWith(`/vaults/${runtime.vaultId}/clients`),
    );
    await page
      .getByRole("button", {
        name: "Open Rust vault owner · server-hosted",
        exact: true,
      })
      .click();
    const firstClient = (await (await attached).json()).client_id;
    await expect(
      page.locator('[data-notes-session-ready="true"]'),
    ).toBeVisible();
    await page.locator('button[data-path="Note.md"]').click();
    await expect(
      page.locator(".cm-content[contenteditable=true]").last(),
    ).toContainText("Native hosted note");
    await expect
      .poll(() =>
        readFile(join(runtime.vault, ".lapis/workspace.json"), "utf8").catch(
          (error: NodeJS.ErrnoException) => {
            if (error.code === "ENOENT") return "";
            throw error;
          },
        ),
      )
      .toContain("Note.md");

    const reattached = page.waitForResponse((response) =>
      response.url().endsWith(`/vaults/${runtime.vaultId}/clients`),
    );
    void reattached.catch(() => {});
    await page.reload();
    await expect(
      page.locator('[data-notes-session-ready="true"]'),
    ).toBeVisible();
    const secondClient = (await (await reattached).json()).client_id;
    expect(secondClient).not.toBe(firstClient);
    await expect(
      page.locator(".cm-content[contenteditable=true]").last(),
    ).toContainText("Native hosted note");
    await expect(
      page.getByRole("heading", { name: "Your vaults", exact: true }),
    ).not.toBeVisible();

    // Browser loss retains its editing claim. Retire only that test-owned
    // client, then exercise the existing explicit recovery before shutdown.
    const current = await page.request.get(`${origin}/api/v3/auth/current`, {
      headers: { "x-lapis-protocol": "3" },
    });
    const { csrf_token } = await current.json();
    const retired = await page.request.delete(
      `${origin}/api/v3/clients/${firstClient}`,
      {
        headers: {
          "x-lapis-protocol": "3",
          "x-lapis-csrf": csrf_token,
          origin,
        },
      },
    );
    expect(retired.status()).toBe(204);
    const problem = await showEditingProblems(page);
    await expect(problem).toBeVisible();
    await problem.click({ button: "right" });
    await page
      .getByRole("menuitem", { name: "Review saved file", exact: true })
      .click();
    await expect(
      page.getByRole("button", { name: /^Current view: preview/ }),
    ).toBeVisible();
    expect(await readFile(join(runtime.vault, "Note.md"), "utf8")).toContain(
      "Native hosted note",
    );
    await problem.click({ button: "right" });
    await page
      .getByRole("menuitem", { name: "Discard editing claim", exact: true })
      .click();
    await expect(problem).toHaveCount(0);
  } catch (error) {
    await info.attach("Rust host log", {
      body: runtime.logs,
      contentType: "text/plain",
    });
    throw error;
  }
});

test("explicit vault management and logout preserve the chooser across reload", async ({
  page,
}) => {
  await page.goto(origin);
  await signInHostedOwner(page, password);
  await page
    .getByRole("button", {
      name: "Open Rust vault owner · server-hosted",
      exact: true,
    })
    .click();
  await expect(page.locator('[data-notes-session-ready="true"]')).toBeVisible();
  await manageVaults(page);
  await page
    .getByRole("button", { name: "Return to Rust vault", exact: true })
    .click();
  await page.reload();
  await expect(page.locator('[data-notes-session-ready="true"]')).toBeVisible();
  await manageVaults(page);
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Your vaults", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", {
      name: "Open Rust vault owner · server-hosted",
      exact: true,
    })
    .click();
  await expect(page.locator('[data-notes-session-ready="true"]')).toBeVisible();
  await signOutHostedWorkspace(page);
  await page.reload();
  await expect(page.getByLabel("Username")).toBeVisible();
  await signInHostedOwner(page, password);
});

test("expired authentication retains only the vault preference until the owner signs in", async ({
  page,
  context,
}) => {
  await page.goto(origin);
  await signInHostedOwner(page, password);
  await page
    .getByRole("button", {
      name: "Open Rust vault owner · server-hosted",
      exact: true,
    })
    .click();
  await expect(page.locator('[data-notes-session-ready="true"]')).toBeVisible();
  await context.clearCookies();
  await page.reload();
  await expect(page.getByLabel("Username")).toBeVisible();
  await page.getByLabel("Username").fill("owner");
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(page.locator('[data-notes-session-ready="true"]')).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Your vaults", exact: true }),
  ).not.toBeVisible();
});
