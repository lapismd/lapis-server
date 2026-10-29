import { expect, type Page, type TestInfo } from "@playwright/test";
import { randomBytes } from "node:crypto";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import type { NativeTaskCommand } from "@lapis-notes/api/native-tasks";
import type {
  ClientDescriptor,
  VaultCommand,
} from "../../crates/lapis-app-protocol/bindings/protocol";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import { installTasksArtifact } from "./tasks-installed-plugin";

export const hostedTask =
  "---\ntype: task\nid: hosted\ntask:\n  status: open\n  inbox: true\n  start: anytime\n---\n\n# Review the hosted migration\n";

export async function createHostedTasksFixture(
  page: Page,
  artifact: string,
  files: Record<string, string> = { "Hosted.md": hostedTask },
) {
  const repository = resolve("../..");
  const root = await mkdtemp(join(repository, ".local/rust-tasks-browser-"));
  const password = randomBytes(32).toString("base64url");
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const service = new RustServerRuntime(
    root,
    repository,
    origin,
    password,
    undefined,
    "Europe/London",
  );
  await mkdir(service.vault, { recursive: true });
  const plugin = await installTasksArtifact(service.vault, artifact);
  for (const [path, text] of Object.entries(files)) {
    const target = join(service.vault, path);
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, text);
  }
  const calls: Array<{ url: string; command: NativeTaskCommand }> = [];
  const runtimeErrors: string[] = [];
  const pluginFailures: string[] = [];
  let moduleUrl: string | undefined;
  function observe(target: Page) {
    target.on("pageerror", (error) => runtimeErrors.push(error.message));
    target.on("console", (message) => {
      if (
        ["error", "warning"].includes(message.type()) &&
        /svelte/i.test(message.text())
      )
        runtimeErrors.push(message.text());
      if (
        message.type() === "error" &&
        /Failed to (load|enable) plugin|Native Task configuration failed/.test(
          message.text(),
        )
      )
        pluginFailures.push(message.text());
    });
    target.on("request", (request) => {
      if (request.url().endsWith("/tasks") && request.method() === "POST")
        calls.push({ url: request.url(), command: request.postDataJSON() });
      if (
        /\/__lapis\/plugins\/[^/]+\/tasks\/[^/]+\/[^/]+\/main\.mjs$/u.test(
          request.url(),
        )
      )
        moduleUrl = request.url();
    });
  }
  observe(page);
  let client: ClientDescriptor;
  async function headers() {
    const response = await page.request.get(`${origin}/api/v3/auth/current`, {
      headers: { "x-lapis-protocol": "3" },
    });
    expect(response.ok()).toBe(true);
    const { csrf_token } = await response.json();
    return { "x-lapis-protocol": "3", "x-lapis-csrf": csrf_token, origin };
  }
  async function openPage(
    target: Page,
    { ready = true, tasks = true }: { ready?: boolean; tasks?: boolean } = {},
  ): Promise<ClientDescriptor> {
    await target.goto(origin);
    await expect(
      target.getByRole("heading", { name: /^(Sign in|Your vaults)$/ }),
    ).toBeVisible();
    if (await target.getByLabel("Username").isVisible()) {
      await target.getByLabel("Username").fill("owner");
      await target.getByLabel("Password").fill(password);
      await target
        .getByRole("button", { name: "Sign in", exact: true })
        .click();
    }
    const admitted = target.waitForResponse(
      (response) =>
        response.url().endsWith(`/vaults/${service.vaultId}/clients`) &&
        response.request().method() === "POST",
    );
    await target
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    const admittedClient: ClientDescriptor = await (await admitted).json();
    if (!ready) return admittedClient;
    await expect(target.getByText("Server DB", { exact: true })).toBeVisible();
    expect(runtimeErrors).toEqual([]);
    if (tasks)
      await target
        .getByRole("button", { name: "Open Tasks", exact: true })
        .click();
    return admittedClient;
  }
  await service.start();
  return {
    service,
    origin,
    headers,
    plugin,
    calls,
    runtimeErrors,
    pluginFailures,
    moduleUrl: () => moduleUrl,
    client: () => client,
    async open(options: { ready?: boolean; tasks?: boolean } = {}) {
      client = await openPage(page, options);
    },
    async openOther(target: Page) {
      observe(target);
      return openPage(target);
    },
    async restart() {
      await page.goto("about:blank");
      await service.restart();
    },
    async references() {
      const response = await page.request.post(
        `${origin}/api/v3/clients/${client.client_id}/database`,
        {
          headers: await headers(),
          data: {
            method: "getMeta",
            args: [
              {
                kind: "value",
                value: "plugin:tasks:native-request-references:v1",
              },
            ],
          },
        },
      );
      expect(response.ok()).toBe(true);
      const result = await response.json();
      expect(result.kind).toBe("value");
      expect(Array.isArray(result.value)).toBe(true);
      return result.value;
    },
    async tasks(command: NativeTaskCommand) {
      const response = await page.request.post(
        `${origin}/api/v3/clients/${client.client_id}/tasks`,
        { headers: await headers(), data: command },
      );
      expect(response.ok()).toBe(true);
      return response.json();
    },
    async execute(command: VaultCommand) {
      const request_id = crypto.randomUUID();
      const response = await page.request.post(
        `${origin}/api/v3/clients/${client.client_id}/commands`,
        {
          headers: await headers(),
          data: {
            protocol_version: 3,
            request_id,
            vault_id: client.vault_id,
            generation: client.generation,
            command,
          },
        },
      );
      return response.json();
    },
    async stop(info: TestInfo, expectedPluginFailures = 0) {
      if (info.status !== info.expectedStatus) {
        if (!page.isClosed())
          await info.attach("Hosted Tasks page before teardown", {
            body: await page.locator("body").ariaSnapshot(),
            contentType: "text/yaml",
          });
        await info.attach("Rust Tasks host log", {
          body: service.logs,
          contentType: "text/plain",
        });
        await info.attach("Rust Tasks calls", {
          body: JSON.stringify(calls),
          contentType: "application/json",
        });
        await info.attach("Rust Tasks runtime errors", {
          body: JSON.stringify({ runtimeErrors, pluginFailures }),
          contentType: "application/json",
        });
      }
      try {
        if (!page.isClosed()) await page.goto("about:blank");
      } finally {
        await service.stop();
        expect(runtimeErrors).toEqual([]);
        expect(pluginFailures).toHaveLength(expectedPluginFailures);
      }
    },
  };
}
