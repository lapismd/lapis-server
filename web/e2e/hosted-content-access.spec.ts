import { expect, test, type Page } from "@playwright/test";
import { randomBytes, randomUUID } from "node:crypto";
import { mkdtemp, mkdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import {
  signInHostedOwner,
  signOutHostedWorkspace,
} from "./rust-hosted-session";

test.use({ trace: "retain-on-failure" });

async function settings(page: Page) {
  await page
    .getByRole("button", {
      name: "Open Rust vault owner · server-hosted",
      exact: true,
    })
    .click();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page
    .getByRole("navigation", { name: "Settings", exact: true })
    .locator("section")
    .filter({
      has: page.getByRole("heading", { name: "Core plugins", exact: true }),
    })
    .getByRole("button", { name: "Content API and MCP", exact: true })
    .click();
  await expect(
    page.getByRole("switch", { name: "Enable API and MCP access" }),
  ).toBeVisible();
}

async function issue(page: Page, permissions: string[] = []) {
  const response = page.waitForResponse(
    (r) =>
      r.url().endsWith("/api/v3/content-access/grants") &&
      r.request().method() === "POST",
  );
  await page
    .getByRole("button", { name: "Create a scoped credential", exact: true })
    .click();
  const created = await response;
  expect(created.request().postDataJSON().permissions ?? []).toEqual(
    permissions,
  );
  expect(created.status()).toBe(201);
  expect(created.headers()["cache-control"]).toBe("no-store");
  const { token } = await created.json();
  await expect(
    page.getByRole("button", { name: /^Copy full credential:/ }),
  ).toBeVisible();
  const storage = await page.evaluate(() =>
    JSON.stringify([localStorage, sessionStorage]),
  );
  expect(storage.includes(token)).toBe(false);
  return token as string;
}

for (const renderer of ["static", "preview", "development"] as const) {
  test(`content plugin controls the real hosted HTTP and MCP endpoints (${renderer})`, async ({
    page,
  }) => {
    const repository = resolve(import.meta.dirname, "../..");
    const browserErrors: string[] = [];
    page.on("pageerror", (error) => browserErrors.push(error.message));
    await mkdir(join(repository, ".local"), { recursive: true });
    const root = await mkdtemp(join(repository, ".local/content-settings-"));
    const password = randomBytes(32).toString("base64url");
    const origin = `http://localhost:${await unusedLoopbackPort()}`;
    const runtime = new RustServerRuntime(
      root,
      repository,
      origin,
      password,
      undefined,
      undefined,
      renderer === "static" ? join(repository, "web/dist") : undefined,
      renderer === "development" ? "development" : "preview",
    );
    const headers = (token: string) => ({
      authorization: `Bearer ${token}`,
      "x-lapis-content-api": "1",
    });
    const read = (token: string, vault = runtime.vaultId) =>
      fetch(`${origin}/api/content/v1/vaults/${vault}/note?path=Note.md`, {
        headers: headers(token),
      });
    try {
      await runtime.start();
      expect(runtime.processCommands).toHaveLength(
        renderer === "static" ? 1 : 2,
      );
      await page.goto(origin);
      await signInHostedOwner(page, password);
      await settings(page);
      const enabled = page.getByRole("switch", {
        name: "Enable API and MCP access",
      });
      for (const title of [
        "Endpoints",
        "Personal access tokens",
        "MCP OAuth",
        "Temporary credentials",
      ]) {
        await expect(
          page.getByRole("heading", { name: title, exact: true }),
        ).toBeVisible();
      }
      await expect(enabled).not.toBeChecked();
      expect((await read("invalid")).status).toBe(404);
      await enabled.click();
      await expect(
        page.getByText("Enabled · scoped access", { exact: true }),
      ).toBeVisible();
      for (const [id, path] of [
        ["api", "/api/content/v1"],
        ["mcp", "/mcp"],
      ]) {
        const field = page.locator(`[data-setting-id="content-access.${id}"]`);
        await expect(field).toHaveAttribute("data-setting-layout", "stacked");
        await expect(
          field.getByText(`${origin}${path}`, { exact: true }),
        ).toBeVisible();
      }
      await page
        .getByRole("textbox", { name: "Credential label", exact: true })
        .fill("Browser acceptance client");
      const credentialVaults = page.getByRole("listbox", {
        name: "Vaults this credential can read",
      });
      const selectedVault = credentialVaults.getByRole("option", {
        name: "Rust vault",
        selected: true,
      });
      await expect(selectedVault).toBeVisible();
      const first = await issue(page);
      const note = await read(first);
      expect(note.status).toBe(200);
      expect(note.headers.get("etag")).toBeTruthy();
      const saved = await readFile(join(runtime.vault, "Note.md"), "utf8");
      expect(await note.text()).toBe(saved);
      expect((await read(first, randomUUID())).status).toBe(403);
      const mcp = await fetch(`${origin}/mcp`, {
        method: "POST",
        headers: {
          ...headers(first),
          "content-type": "application/json",
          accept: "application/json, text/event-stream",
          "mcp-protocol-version": "2026-07-28",
          "mcp-method": "tools/call",
          "mcp-name": "read_note",
        },
        body: JSON.stringify({
          jsonrpc: "2.0",
          id: 1,
          method: "tools/call",
          params: {
            name: "read_note",
            arguments: { vault_id: runtime.vaultId, path: "Note.md" },
            _meta: {
              "io.modelcontextprotocol/protocolVersion": "2026-07-28",
              "io.modelcontextprotocol/clientCapabilities": {},
            },
          },
        }),
      });
      expect(mcp.status).toBe(200);
      expect(
        (await mcp.json()).result.structuredContent.outcome.value.text,
      ).toBe(saved);
      await page
        .getByRole("button", {
          name: "Hide the one-time credential",
          exact: true,
        })
        .click();
      await expect(
        page.getByText("No credential to display", { exact: true }),
      ).toBeVisible();
      await page.getByRole("button", { name: "Revoke", exact: true }).click();
      await expect(
        page.getByRole("button", { name: "Revoke", exact: true }),
      ).toHaveCount(0);
      expect((await read(first)).status).toBe(401);
      await page
        .getByRole("combobox", {
          name: "Plugin operation permissions",
          exact: true,
        })
        .click();
      await page
        .getByRole("option", { name: "tasks:read", exact: true })
        .click();
      await page.keyboard.press("Escape");
      const second = await issue(page, ["tasks:read"]);
      if (renderer === "static") {
        await page
          .getByRole("combobox", { name: "Personal access token expiry" })
          .click();
        await page.getByRole("option", { name: "Never", exact: true }).click();
      }
      await page
        .getByRole("textbox", { name: "Token label", exact: true })
        .fill("Persistent browser client");
      await page
        .getByRole("combobox", { name: "Token plugin permissions" })
        .click();
      await page
        .getByRole("option", { name: "notes:write", exact: true })
        .click();
      await page.keyboard.press("Escape");
      const issuedPersonal = page.waitForResponse(
        (response) =>
          response.url().endsWith("/api/v3/content-access/tokens") &&
          response.request().method() === "POST",
      );
      await page
        .getByRole("button", { name: "Create a personal access token" })
        .click();
      const personalResponse = await issuedPersonal;
      expect(personalResponse.status()).toBe(201);
      expect(personalResponse.request().postDataJSON()).toMatchObject({
        label: "Persistent browser client",
        lifetime: renderer === "static" ? "never" : "days90",
        permissions: ["notes:write"],
      });
      await expect(
        page.getByRole("button", { name: /^Copy full personal access token:/ }),
      ).toBeVisible();
      await expect(
        page.getByText("No credential to display", { exact: true }),
      ).toBeVisible();
      const { token: personal } = await personalResponse.json();
      expect((await read(personal)).status).toBe(200);
      await signOutHostedWorkspace(page);
      expect((await read(second)).status).toBe(200);
      // Dispose the old development document before Vite restarts: its HMR
      // reconnect handler reloads the page and can race the next navigation.
      await page.goto("about:blank");
      await runtime.restart();
      await page.goto(origin);
      await signInHostedOwner(page, password);
      await settings(page);
      await expect(enabled).toBeChecked();
      expect((await read(second)).status).toBe(401);
      expect((await read(personal)).status).toBe(200);
      const third = await issue(page);
      await enabled.click();
      await expect(page.getByText("Disabled", { exact: true })).toBeVisible();
      expect((await read(third)).status).toBe(404);
      await enabled.click();
      await expect(
        page.getByText("Enabled · scoped access", { exact: true }),
      ).toBeVisible();
      expect((await read(third)).status).toBe(401);
      expect((await read(personal)).status).toBe(401);
      await enabled.click();
      await expect(page.getByText("Disabled", { exact: true })).toBeVisible();
      await signOutHostedWorkspace(page);
      await page.goto("about:blank");
      await runtime.restart();
      expect((await read(third)).status).toBe(404);
      expect(
        runtime.logs.includes(first) ||
          runtime.logs.includes(second) ||
          runtime.logs.includes(third) ||
          runtime.logs.includes(personal),
      ).toBe(false);
      expect(browserErrors).toEqual([]);
    } catch (error) {
      await test.info().attach("host-and-browser-diagnostics", {
        body: JSON.stringify({
          browserErrors,
          url: page.url(),
          logs: runtime.logs,
        }),
        contentType: "application/json",
      });
      throw error;
    } finally {
      await runtime.stop();
    }
  });
}
