import { expect, test, type Page } from "@playwright/test";
import { randomBytes, randomUUID } from "node:crypto";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { mkdtemp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import {
  signInHostedOwner,
  signOutHostedWorkspace,
} from "./rust-hosted-session";

const execute = promisify(execFile);
async function command(page: Page, name: string) {
  const action = name.startsWith("Install") ? "install" : "update";
  const completed = page.waitForResponse(
    (response) =>
      response.url().endsWith(`/plugins/skills/operations/${action}`) &&
      response.request().method() === "POST",
  );
  await page
    .getByRole("button", { name: "Open command palette", exact: true })
    .click();
  const palette = page.getByRole("dialog", { name: "Command Palette" });
  await palette.getByRole("combobox", { name: "Search commands" }).fill(name);
  await palette.getByRole("option", { name: new RegExp(name) }).click();
  await expect(palette).toHaveCount(0);
  const response = await completed;
  expect(response.status()).toBe(200);
  const reply = await response.json();
  expect(reply.outcome.status).toBe("ok");
  return reply.outcome.value as Array<{
    name: string;
    status: string;
    reason?: string;
  }>;
}
async function openVault(page: Page) {
  await page
    .getByRole("button", {
      name: "Open Rust vault owner · server-hosted",
      exact: true,
    })
    .click();
  await expect(
    page.locator('[data-ui-component="lapis-workspace-shell"]'),
  ).toBeVisible();
}
async function credential(page: Page, permissions: string[]) {
  const selector = page.locator("button").and(
    page.getByRole("combobox", {
      name: "Plugin operation permissions",
      exact: true,
    }),
  );
  if (permissions.length) {
    await selector.click();
    for (const permission of permissions)
      await page.getByRole("option", { name: permission, exact: true }).click();
    await selector.click();
    await expect(selector).toHaveAttribute("aria-expanded", "false");
  }
  const response = page.waitForResponse(
    (r) =>
      r.url().endsWith("/api/v3/content-access/grants") &&
      r.request().method() === "POST",
  );
  await page
    .getByRole("button", { name: "Create a scoped credential", exact: true })
    .click();
  const created = await response;
  expect(created.status()).toBe(201);
  expect(created.request().postDataJSON().permissions ?? []).toEqual(
    permissions,
  );
  const { token } = await created.json();
  await page
    .getByRole("button", { name: "Hide the one-time credential", exact: true })
    .click();
  return token as string;
}

test("app and CLI protect packaged references in the same vault across cold restart", async ({
  page,
}) => {
  const repository = resolve(import.meta.dirname, "../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  const root = await mkdtemp(join(repository, ".local/hosted-skills-"));
  const password = randomBytes(32).toString("base64url");
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const bundled = join(root, "bundled-skills.json");
  await execute(
    process.execPath,
    [join(repository, "scripts/prepare-bundled-skills.mjs"), bundled],
    { cwd: repository },
  );
  const previous = process.env.LAPIS_SERVER_BUNDLED_SKILLS;
  process.env.LAPIS_SERVER_BUNDLED_SKILLS = bundled;
  const runtime = new RustServerRuntime(
    root,
    repository,
    origin,
    password,
    undefined,
    undefined,
    join(repository, "web/dist"),
  );
  const cli = (file: string, action: string) =>
    execute(join(repository, ".local/rust-core-target/debug/lapis"), [
      "--target",
      origin,
      "--token-file",
      file,
      "--json",
      "skills",
      action,
      "--vault",
      runtime.vaultId,
    ]);
  const reference = () =>
    readFile(
      join(runtime.vault, ".agents/skills/lapis-cli/references/commands.md"),
      "utf8",
    );
  try {
    await runtime.start();
    await page.goto(origin);
    await signInHostedOwner(page, password);
    await openVault(page);
    // The owner command works independently of the external API or AI plugin.
    await command(page, "Install plugin skills");
    await expect
      .poll(async () => {
        try {
          return await reference();
        } catch {
          return "";
        }
      })
      .toContain("Lapis CLI commands and discovery");
    const original = await reference();
    const receipt = JSON.parse(
      await readFile(join(runtime.vault, ".agents/.lapis-skills.json"), "utf8"),
    );
    expect(
      Object.hasOwn(
        receipt.skills["lapis-cli"].files,
        "references/commands.md",
      ),
    ).toBe(true);
    await expect
      .poll(async () => {
        try {
          return await readFile(
            join(
              runtime.vault,
              ".agents/skills/lapis-markdown/references/RICH_SYNTAX.md",
            ),
            "utf8",
          );
        } catch {
          return "";
        }
      })
      .toContain("Grid tables and spans");

    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await page
      .getByRole("navigation", { name: "Settings", exact: true })
      .locator("section")
      .filter({
        has: page.getByRole("heading", { name: "Core plugins", exact: true }),
      })
      .getByRole("button", { name: "Content API and MCP", exact: true })
      .click();
    await page
      .getByRole("switch", { name: "Enable API and MCP access" })
      .click();
    console.info("skills acceptance: issuing read credential");
    const readOnly = join(root, "read-only-credential");
    await writeFile(readOnly, await credential(page, []), { mode: 0o600 });
    // The live CLI catalog omits ungranted plugins (Unsupported, exit 10).
    await expect(cli(readOnly, "update")).rejects.toMatchObject({ code: 10 });
    // Direct invocation independently enforces write authority as HTTP 403.
    console.info("skills acceptance: checking direct write denial");
    const denied = await fetch(
      `${origin}/api/content/v1/vaults/${runtime.vaultId}/plugins/skills/operations/update`,
      {
        method: "POST",
        headers: {
          authorization: `Bearer ${await readFile(readOnly, "utf8")}`,
          "x-lapis-content-api": "1",
          "content-type": "application/json",
        },
        body: JSON.stringify({
          protocol_version: 1,
          version: 1,
          request_id: randomUUID(),
          input: {},
        }),
      },
    );
    expect(denied.status).toBe(403);
    expect(await reference()).toBe(original);
    console.info("skills acceptance: issuing write credential");
    const writer = join(root, "skill-credential");
    await writeFile(
      writer,
      await credential(page, ["skills:read", "skills:write"]),
      { mode: 0o600 },
    );
    console.info("skills acceptance: CLI list");
    const list = JSON.parse((await cli(writer, "list")).stdout).outcome.value;
    expect(
      list.find((s: { name: string }) => s.name === "lapis-cli").status,
    ).toBe("unchanged");
    const edited = original + "\nLocal command guidance retained.\n";
    await writeFile(
      join(runtime.vault, ".agents/skills/lapis-cli/references/commands.md"),
      edited,
    );
    await page
      .getByRole("button", { name: "Close settings", exact: true })
      .click();
    console.info("skills acceptance: app update");
    const appUpdate = await command(page, "Update plugin skills");
    expect(appUpdate.find((skill) => skill.name === "lapis-cli")).toMatchObject(
      { status: "skipped", reason: "locally-modified" },
    );
    expect(await reference()).toBe(edited);
    console.info("skills acceptance: CLI update");
    const update = JSON.parse((await cli(writer, "update")).stdout).outcome
      .value;
    expect(
      update.find((s: { name: string }) => s.name === "lapis-cli"),
    ).toMatchObject({ status: "skipped", reason: "locally-modified" });
    console.info("skills acceptance: sign out");
    await signOutHostedWorkspace(page);
    await page.goto("about:blank");
    console.info("skills acceptance: cold restart");
    await runtime.restart();
    // Credential sessions do not survive a cold host; receipts and local edits do.
    await expect(cli(writer, "list")).rejects.toMatchObject({ code: 3 });
    await page.goto(origin);
    await signInHostedOwner(page, password);
    await openVault(page);
    console.info("skills acceptance: app update");
    const coldUpdate = await command(page, "Update plugin skills");
    expect(
      coldUpdate.find((skill) => skill.name === "lapis-cli"),
    ).toMatchObject({ status: "skipped", reason: "locally-modified" });
    expect(await reference()).toBe(edited);
    console.info("skills acceptance: sign out");
    await signOutHostedWorkspace(page);
  } finally {
    try {
      if (!page.isClosed()) await page.goto("about:blank");
    } finally {
      try {
        await runtime.stop();
      } finally {
        if (previous === undefined)
          delete process.env.LAPIS_SERVER_BUNDLED_SKILLS;
        else process.env.LAPIS_SERVER_BUNDLED_SKILLS = previous;
        await rm(root, { recursive: true, force: true });
      }
    }
  }
});
