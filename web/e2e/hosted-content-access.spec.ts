import { expect, test, type Page } from "@playwright/test";
import { randomBytes, randomUUID } from "node:crypto";
import { mkdtemp, mkdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import {
  signInHostedOwner,
  signOutHostedWorkspace,
} from "./rust-hosted-session";

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

async function issue(page: Page) {
  const response = page.waitForResponse(
    (r) =>
      r.url().endsWith("/api/v3/content-access/grants") &&
      r.request().method() === "POST",
  );
  await page
    .getByRole("button", { name: "Create a read-only credential", exact: true })
    .click();
  const created = await response;
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
      await expect(enabled).not.toBeChecked();
      expect((await read("invalid")).status).toBe(404);
      await enabled.click();
      await expect(
        page.getByText("Enabled · read-only", { exact: true }),
      ).toBeVisible();
      await page
        .getByRole("textbox", { name: "Credential label", exact: true })
        .fill("Browser acceptance client");
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
      const second = await issue(page);
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
      const third = await issue(page);
      await enabled.click();
      await expect(page.getByText("Disabled", { exact: true })).toBeVisible();
      expect((await read(third)).status).toBe(404);
      await enabled.click();
      await expect(
        page.getByText("Enabled · read-only", { exact: true }),
      ).toBeVisible();
      expect((await read(third)).status).toBe(401);
      await enabled.click();
      await expect(page.getByText("Disabled", { exact: true })).toBeVisible();
      await signOutHostedWorkspace(page);
      await page.goto("about:blank");
      await runtime.restart();
      expect((await read(third)).status).toBe(404);
      expect(
        runtime.logs.includes(first) ||
          runtime.logs.includes(second) ||
          runtime.logs.includes(third),
      ).toBe(false);
    } finally {
      await runtime.stop();
    }
  });
}
