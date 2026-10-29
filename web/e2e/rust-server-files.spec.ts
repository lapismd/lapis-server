import { expect, test, type Page } from "@playwright/test";
import { randomBytes } from "node:crypto";
import { access, mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import {
  signInHostedOwner,
  signOutHostedWorkspace,
} from "./rust-hosted-session";

let service: RustServerRuntime;
let origin: string;

test.beforeEach(async ({ page }) => {
  const repository = resolve("../..");
  const root = await mkdtemp(join(repository, ".local/rust-files-browser-"));
  const password = randomBytes(32).toString("base64url");
  origin = `http://localhost:${await unusedLoopbackPort()}`;
  service = new RustServerRuntime(root, repository, origin, password);
  await service.start();
  await mkdir(join(service.vault, "Archive"));
  await mkdir(join(service.vault, "Moved"));
  await writeFile(join(service.vault, "Note.md"), "# Original note\n");
  await writeFile(join(service.vault, "Keep.md"), "# Keep destination\n");
  await writeFile(
    join(service.vault, "Archive/Renamed.md"),
    "# Keep archive\n",
  );
  await page.goto(origin);
  await signInHostedOwner(page, password);
  await page
    .getByRole("button", { name: "Open Rust vault", exact: true })
    .click();
  await expect(page.getByText("Server DB", { exact: true })).toBeVisible();
});

test.afterEach(async ({ page }, testInfo) => {
  if (testInfo.status !== testInfo.expectedStatus)
    await testInfo.attach("Rust file-action host log", {
      body: service.logs,
      contentType: "text/plain",
    });
  try {
    await signOutHostedWorkspace(page);
  } finally {
    await service?.stop();
  }
});

async function rename(page: Page, path: string, name: string): Promise<void> {
  await page.locator(`button[data-path="${path}"]`).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Rename...", exact: true }).click();
  const input = page.getByRole("textbox", { name: "Rename...", exact: true });
  await input.fill(name);
  await input.press("Enter");
  await expect(input).not.toBeVisible();
}

async function exists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return false;
    throw error;
  }
}

test("retained Explorer rename and move preserve collisions, editor identity and cold layout", async ({
  page,
}) => {
  await page.locator('button[data-path="Note.md"]').click();
  const editor = page.locator(".cm-content[contenteditable=true]").last();
  await expect(editor).toBeVisible();
  await rename(page, "Note.md", "Keep.md");
  await expect(
    page.getByText("A file already exists at /Keep.md", { exact: true }),
  ).toBeVisible();
  expect(await readFile(join(service.vault, "Keep.md"), "utf8")).toBe(
    "# Keep destination\n",
  );
  expect(await readFile(join(service.vault, "Note.md"), "utf8")).toBe(
    "# Original note\n",
  );
  const draft = "# Saved through Rust rename\n";
  await editor.fill(draft);
  await rename(page, "Note.md", "Renamed.md");
  const renamed = page.locator('button[data-path="Renamed.md"]');
  await expect(renamed).toBeVisible();
  expect(await exists(join(service.vault, "Note.md"))).toBe(false);
  expect(await readFile(join(service.vault, "Renamed.md"), "utf8")).toBe(draft);
  await renamed.dragTo(page.locator('button[data-path="Archive"]'));
  await expect(
    page.getByText("A file already exists at Archive/Renamed.md", {
      exact: true,
    }),
  ).toBeVisible();
  expect(
    await readFile(join(service.vault, "Archive/Renamed.md"), "utf8"),
  ).toBe("# Keep archive\n");
  expect(await readFile(join(service.vault, "Renamed.md"), "utf8")).toBe(draft);
  await renamed.dragTo(page.locator('button[data-path="Moved"]'));
  await expect(
    page.locator('button[data-path="Moved/Renamed.md"]'),
  ).toBeVisible();
  const movedDraft = "# Saved after Rust move\n";
  await editor.fill(movedDraft);
  await expect
    .poll(() => readFile(join(service.vault, "Moved/Renamed.md"), "utf8"))
    .toBe(movedDraft);
  expect(await exists(join(service.vault, "Renamed.md"))).toBe(false);
  const layout = join(service.vault, ".lapis/workspace.json");
  await expect
    .poll(async () => ((await exists(layout)) ? readFile(layout, "utf8") : ""))
    .toContain("Moved/Renamed.md");
  await signOutHostedWorkspace(page);
  await service.restart();
  await page.reload();
  await signInHostedOwner(page, service.password);
  await page
    .getByRole("button", { name: "Open Rust vault", exact: true })
    .click();
  await expect(editor).toContainText("Saved after Rust move");
});

test("retained Explorer creates, renames and locally trashes a Unicode note through Rust", async ({
  page,
}) => {
  await page.getByRole("button", { name: "Create File", exact: true }).click();
  await expect(page.locator('button[data-path="Untitled.md"]')).toBeVisible();
  const editor = page.locator(".cm-content[contenteditable=true]").last();
  await editor.fill("# Portable café\n");
  await rename(page, "Untitled.md", "Created café.md");
  const created = page.locator('button[data-path="Created café.md"]');
  await expect(created).toBeVisible();
  expect(await readFile(join(service.vault, "Created café.md"), "utf8")).toBe(
    "# Portable café\n",
  );
  await created.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Delete", exact: true }).click();
  await expect(created).not.toBeVisible();
  expect(await exists(join(service.vault, "Created café.md"))).toBe(false);
  expect(
    await readFile(join(service.vault, ".trash/Created café.md"), "utf8"),
  ).toBe("# Portable café\n");
});
