import { expect, type Page } from "@playwright/test";
import { randomUUID } from "node:crypto";
import {
  copyFile,
  mkdtemp,
  mkdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { build } from "vite";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";

const repository = resolve(import.meta.dirname, "../..");
const hostUrl = "http://127.0.0.1:58191/";
const sandboxUrl = "http://127.0.0.1:58192/sandbox";
const MIME = "text/html;profile=mcp-app";

export async function bundleExternalHost() {
  const result = await build({
    configFile: false,
    logLevel: "warn",
    build: {
      write: false,
      target: "es2022",
      modulePreload: false,
      rollupOptions: {
        input: join(import.meta.dirname, "mcp-apps-host.ts"),
        output: { inlineDynamicImports: true },
      },
    },
  });
  const bundle = Array.isArray(result) ? result[0] : result;
  if (!("output" in bundle)) throw new Error("Expected in-memory host bundle");
  const entry = bundle.output.find((item) => item.type === "chunk");
  if (!entry || entry.type !== "chunk")
    throw new Error("External host bundle unavailable");
  return entry.code;
}

export async function startMcpAppsFixture({ includeProviders = true } = {}) {
  const root = await mkdtemp(join(tmpdir(), "lapis-mcp-apps-"));
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const password = randomUUID();
  const web = join(root, "web");
  const server = new RustServerRuntime(
    root,
    repository,
    origin,
    password,
    undefined,
    undefined,
    web,
  );
  try {
    await mkdir(web);
    await writeFile(
      join(web, "index.html"),
      "<!doctype html><title>Fixture</title>",
    );
    await mkdir(server.vault);
    await writeFile(
      join(server.vault, "Note.md"),
      "# Saved note\n\n**Important** [link](https://example.invalid/leak) ![private image](https://example.invalid/image)\n\n<script>parent.location = 'https://example.invalid/script'</script>\n",
    );
    await writeFile(
      join(server.vault, "One.md"),
      "---\ntype: task\nid: one\ntask:\n  status: open\n  inbox: false\n  start: anytime\n  plan:\n    date: 2026-08-20\n    at: '09:00'\n    duration: 30m\n---\n\n# One-off task\n",
    );
    const fixtures = JSON.parse(
      await readFile(
        join(
          repository,
          "crates/lapis-app-core/tests/fixtures/task-lifecycle-duration.json",
        ),
        "utf8",
      ),
    );
    const recurring = fixtures.cases.find(
      (entry: { name: string }) =>
        entry.name === "valid duration / complete / schedule / duration",
    );
    await writeFile(join(server.vault, "Read.md"), recurring.raw);
    const prev = process.env.LAPIS_SERVER_MCP_APPS_ROOT;
    let mcpApps = join(repository, ".local/mcp-apps-dist");
    if (!includeProviders) {
      const notesOnly = join(root, "mcp-apps");
      await mkdir(notesOnly);
      await copyFile(
        join(mcpApps, "notes.html"),
        join(notesOnly, "notes.html"),
      );
      mcpApps = notesOnly;
    }
    process.env.LAPIS_SERVER_MCP_APPS_ROOT = mcpApps;
    try {
      await server.start();
    } finally {
      if (prev === undefined) delete process.env.LAPIS_SERVER_MCP_APPS_ROOT;
      else process.env.LAPIS_SERVER_MCP_APPS_ROOT = prev;
    }

    const ownerHeaders = {
      "content-type": "application/json",
      "x-lapis-protocol": "3",
      origin,
    };
    const login = await fetch(origin + "/api/v3/auth/login", {
      method: "POST",
      headers: ownerHeaders,
      body: JSON.stringify({ username: "owner", password }),
    });
    expect(login.ok).toBe(true);
    const csrf = (await login.json()).csrf_token;
    const cookie = login.headers.getSetCookie()[0].split(";")[0];
    const send = async (path: string, data: unknown, method = "POST") => {
      const response = await fetch(origin + path, {
        method,
        headers: { ...ownerHeaders, cookie, "x-lapis-csrf": csrf },
        body: data === undefined ? undefined : JSON.stringify(data),
      });
      expect(response.ok, path).toBe(true);
      return response.status === 204 ? null : response.json();
    };
    const { client_id, generation } = await send(
      `/api/v3/vaults/${server.vaultId}/clients`,
      { protocol_version: 3 },
    );
    const client = `/api/v3/clients/${client_id}`;
    function command(command: Record<string, unknown>) {
      return send(client + "/commands", {
        protocol_version: 3,
        request_id: randomUUID(),
        vault_id: server.vaultId,
        generation,
        command,
      });
    }

    await send(client + "/tasks", {
      kind: "configure",
      configuration: {
        day_parts: { afternoonStartsAt: "12:00", eveningStartsAt: "17:00" },
        daily: { folder: "daily", date_format: "yyyy-LL-dd" },
        format: { locale: "en-US", firstDay: 7, minimalDays: 4 },
        provider: { kind: "default" },
      },
    });
    await send(client + "/indexing", { kind: "refresh", rebuild: false });
    await send("/api/v3/content-access", { enabled: true });
    function grant(permissions: string[]) {
      return send("/api/v3/content-access/grants", {
        label: "External host",
        vault_ids: [server.vaultId],
        lifetime_seconds: 3600,
        permissions,
      });
    }
    async function rpc(
      token: string,
      method: string,
      params: Record<string, unknown>,
    ) {
      const name = params.name ?? params.uri;
      const response = await fetch(origin + "/mcp", {
        method: "POST",
        headers: {
          accept: "application/json, text/event-stream",
          "content-type": "application/json",
          authorization: `Bearer ${token}`,
          "mcp-protocol-version": "2026-07-28",
          "mcp-method": method,
          ...(typeof name === "string" ? { "mcp-name": name } : {}),
        },
        body: JSON.stringify({
          jsonrpc: "2.0",
          id: 1,
          method,
          params: {
            ...params,
            _meta: {
              "io.modelcontextprotocol/protocolVersion": "2026-07-28",
              "io.modelcontextprotocol/clientCapabilities": {
                extensions: {
                  "io.modelcontextprotocol/ui": { mimeTypes: [MIME] },
                },
              },
            },
          },
        }),
      });
      if (!response.ok)
        throw new Error(`MCP request failed: ${response.status}`);
      const message = await response.json();
      if (message.error) throw new Error(`MCP error: ${message.error.code}`);
      return message.result;
    }
    return {
      server,
      origin,
      send,
      client,
      command,
      grant,
      rpc,
      async stop() {
        try {
          await server.stop();
        } finally {
          await rm(root, { recursive: true, force: true });
        }
      },
    };
  } catch (error) {
    await server.stop().catch(() => {});
    await rm(root, { recursive: true, force: true });
    throw error;
  }
}

export async function mountMcpApp(
  page: Page,
  code: string,
  fixture: Awaited<ReturnType<typeof startMcpAppsFixture>>,
  token: string,
  tool: string,
  args: Record<string, unknown>,
  loseCompletion?: (name: string, args: Record<string, unknown>) => boolean,
) {
  const sandbox = await readFile(
    join(import.meta.dirname, "mcp-apps-sandbox.html"),
    "utf8",
  );
  await page.route(hostUrl, (route) =>
    route.fulfill({
      contentType: "text/html",
      body: `<!doctype html><meta name="viewport" content="width=device-width"><script type="module">${code.replaceAll("</script", "<\\/script")}</script>`,
    }),
  );
  await page.route(sandboxUrl, (route) =>
    route.fulfill({ contentType: "text/html", body: sandbox }),
  );
  await page.exposeFunction(
    "relayTool",
    async (name: string, input: Record<string, unknown>) => {
      const result = await fixture.rpc(token, "tools/call", {
        name,
        arguments: input,
      });
      if (loseCompletion?.(name, input))
        throw new Error("Response lost after server dispatch");
      return result;
    },
  );
  const { tools } = await fixture.rpc(token, "tools/list", {});
  const uri = tools.find((entry: { name: string }) => entry.name === tool)._meta
    .ui.resourceUri;
  const resource = await fixture.rpc(token, "resources/read", { uri });
  expect(resource.contents[0].mimeType).toBe(MIME);
  const result = await fixture.rpc(token, "tools/call", {
    name: tool,
    arguments: args,
  });
  await page.goto(hostUrl);
  await page.evaluate(
    (data) =>
      window.postMessage({ type: "open-app", ...data }, location.origin),
    {
      html: resource.contents[0].text,
      args,
      result,
    },
  );
  return page
    .frameLocator('iframe[title="External sandbox"]')
    .frameLocator('iframe[title="MCP App"]');
}
