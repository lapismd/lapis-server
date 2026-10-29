import { test, expect } from "@playwright/test";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { randomBytes } from "node:crypto";
import { resolve, join } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";

test("retained backup settings and passive status preserve saved selection through lost acknowledgement and cold restart", async ({
  page,
}, testInfo) => {
  const repository = resolve("../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  const root = await mkdtemp(join(repository, ".local/rust-hosted-backup-"));
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const password = randomBytes(32).toString("base64url");
  const runtime = new RustServerRuntime(root, repository, origin, password);
  const path = join(runtime.vault, ".lapis/app.json");
  await mkdir(join(runtime.vault, ".lapis"), { recursive: true });
  await writeFile(
    path,
    JSON.stringify({
      keep: "retained",
      pluginData: {
        other: { value: 2 },
        "repository-history": {
          automaticBackup: false,
          showWorkingCopyStatus: false,
        },
      },
    }),
  );
  const controls: string[] = [];
  page.on("request", (request) => {
    if (
      request.method() === "POST" &&
      request.url().endsWith("/repository-backup")
    )
      controls.push(request.postDataJSON().kind);
  });
  const saved = async () => JSON.parse(await readFile(path, "utf8"));
  const open = async () => {
    await page.goto(origin);
    await expect(
      page.getByRole("heading", { name: /^(Sign in|Your vaults)$/u }),
    ).toBeVisible();
    if (await page.getByLabel("Username").count()) {
      await page.getByLabel("Username").fill("owner");
      await page.getByLabel("Password").fill(password);
      await page.getByRole("button", { name: "Sign in", exact: true }).click();
    }
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await expect(page.getByText("Server DB", { exact: true })).toBeVisible();
  };
  const settings = async () => {
    await page
      .getByRole("button", { name: "Open settings", exact: true })
      .click();
    const dialog = page.getByRole("dialog", { name: "Settings", exact: true });
    await dialog
      .getByRole("button", { name: "Repository History", exact: true })
      .click();
    return dialog;
  };
  try {
    await runtime.start();
    await open();
    let dialog = await settings();
    const automatic = () =>
      dialog.getByRole("switch", {
        name: "Automatic local history",
        exact: true,
      });
    await expect(automatic()).not.toBeChecked();
    await dialog
      .getByRole("switch", { name: "Show working copy status", exact: true })
      .click();
    await expect
      .poll(
        async () =>
          (await saved()).pluginData["repository-history"]
            .showWorkingCopyStatus,
      )
      .toBe(true);
    expect(
      (await saved()).pluginData["repository-history"].automaticBackup,
    ).toBe(false);
    expect(controls).toEqual([]);
    await dialog
      .getByRole("button", { name: "Close settings", exact: true })
      .click();
    await expect(
      page.getByRole("button", {
        name: "Vault history: History paused",
        exact: true,
      }),
    ).toBeVisible();
    dialog = await settings();
    await automatic().click();
    await expect
      .poll(
        async () =>
          (await saved()).pluginData["repository-history"].automaticBackup,
      )
      .toBe(true);
    await dialog
      .getByRole("button", { name: "Close settings", exact: true })
      .click();
    await expect(
      page.getByRole("button", {
        name: "Vault history: Saved locally",
        exact: true,
      }),
    ).toBeVisible();
    await page
      .getByRole("button", { name: "Save vault checkpoint now", exact: true })
      .click();
    await expect
      .poll(() => controls.filter((kind) => kind === "capture").length)
      .toBe(1);
    dialog = await settings();
    await page.route("**/repository-backup", async (route) => {
      if (route.request().postDataJSON().kind !== "set_enabled")
        return route.continue();
      const response = await route.fetch();
      expect(response.ok()).toBe(true);
      await route.abort("failed");
    });
    await automatic().click();
    await expect
      .poll(
        async () =>
          (await saved()).pluginData["repository-history"].automaticBackup,
      )
      .toBe(false);
    await expect(automatic()).not.toBeChecked();
    expect(controls.filter((kind) => kind === "set_enabled")).toHaveLength(2);
    await page.unroute("**/repository-backup");
    await dialog
      .getByRole("button", { name: "Close settings", exact: true })
      .click();
    await expect(
      page.getByRole("button", {
        name: "Vault history: History paused",
        exact: true,
      }),
    ).toBeVisible();
    expect(await saved()).toMatchObject({
      keep: "retained",
      pluginData: {
        other: { value: 2 },
        "repository-history": {
          automaticBackup: false,
          showWorkingCopyStatus: true,
        },
      },
    });
    await page.goto("about:blank");
    await writeFile(
      join(runtime.vault, "Note.md"),
      "saved while history is disabled\n",
    );
    await runtime.restart();
    const before = controls.length;
    await open();
    await expect(
      page.getByRole("button", {
        name: "Vault history: History paused",
        exact: true,
      }),
    ).toBeVisible();
    dialog = await settings();
    await expect(automatic()).not.toBeChecked();
    await dialog
      .getByRole("button", { name: "Close settings", exact: true })
      .click();
    expect(controls.slice(before).every((kind) => kind === "status")).toBe(
      true,
    );
    expect(await readFile(join(runtime.vault, "Note.md"), "utf8")).toBe(
      "saved while history is disabled\n",
    );
    await page.goto("about:blank");
  } catch (error) {
    await testInfo.attach("Rust backup host log", {
      body: runtime.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    await runtime.stop();
  }
});
