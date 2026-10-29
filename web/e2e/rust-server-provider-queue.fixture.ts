import { expect, type Page } from "@playwright/test";
import { createHash, randomBytes } from "node:crypto";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import type {
  QueueCommand,
  QueueRecord,
  TurnRecord,
} from "../../crates/lapis-app-protocol/bindings/protocol";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";

export const activeChat = (page: Page) =>
  page.locator('[data-native-chat="true"]:visible');
export const composer = (page: Page) =>
  activeChat(page)
    .getByRole("group", { name: "Message composer", exact: true })
    .getByRole("textbox", { name: "Message", exact: true });

/** Private opt-in provider fixture; no config or credentials enter its receipt. */
export async function providerQueueFixture() {
  const configuration = process.env.LAPIS_SERVER_TEST_AGENTS_CONFIG;
  if (!configuration)
    throw new Error("Set LAPIS_SERVER_TEST_AGENTS_CONFIG for host acceptance");
  const bytes = await readFile(resolve(configuration));
  const declared = JSON.parse(bytes.toString());
  expect(declared).toHaveLength(1);
  expect(declared[0].default_mode).toBe("read-only");
  expect(declared[0].queue_selection).toEqual({
    recipient: {
      id: "codex",
      label: "Codex",
      token: "jj-store-agent:636f646578;",
    },
    target: { id: "codex", agent: "codex" },
  });
  const repository = resolve("../..");
  const root = await mkdtemp(join(repository, ".local/rust-provider-queue-"));
  const agents = join(root, "agents.json");
  await writeFile(agents, bytes, { mode: 0o600 });
  const password = randomBytes(32).toString("base64url");
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const service = new RustServerRuntime(
    root,
    repository,
    origin,
    password,
    agents,
  );
  await mkdir(join(service.vault, ".lapis"), { recursive: true });
  await writeFile(
    join(service.vault, ".lapis/core-plugins.json"),
    JSON.stringify({ disabled: [], enabled: ["ai"] }),
  );
  const stages: Array<Record<string, unknown>> = [];
  const mutations: Array<{ path: string; command: any }> = [];
  const admittedFixtureCommands: QueueCommand[] = [];
  let queueAdmission: { url: string; csrf: string } | undefined;
  const turn = async (id: string): Promise<TurnRecord> =>
    JSON.parse(
      await readFile(
        join(root, "profile/turns", service.vaultId, `${id}.json`),
        "utf8",
      ),
    );
  const queue = async (id: string): Promise<QueueRecord> =>
    JSON.parse(
      await readFile(
        join(root, "profile/turns/queues", service.vaultId, `${id}.json`),
        "utf8",
      ),
    );
  const transcriptPath = (id: string) =>
    join(service.vault, ".lapis/agents/sessions", id, "transcript.jsonl");
  const transcriptHash = async (id: string) =>
    createHash("sha256")
      .update(await readFile(transcriptPath(id)))
      .digest("hex");
  return {
    root,
    service,
    turn,
    queue,
    mutations,
    admittedFixtureCommands,
    transcriptPath,
    transcriptHash,
    async save(stage: string, facts: Record<string, unknown> = {}) {
      stages.push({ stage, at: new Date().toISOString(), ...facts });
      await writeFile(
        join(root, "receipt.json"),
        JSON.stringify(
          { root, stages, mutations, admittedFixtureCommands },
          null,
          2,
        ),
      );
    },
    observe(page: Page) {
      queueAdmission = undefined;
      page.on("request", (request) => {
        if (request.method() !== "POST") return;
        const path = new URL(request.url()).pathname;
        if (!/\/(?:turns|queues)$/u.test(path)) return;
        if (path.endsWith("/queues")) {
          const csrf = request.headers()["x-lapis-csrf"];
          if (csrf) queueAdmission = { url: request.url(), csrf };
        }
        const command = request.postDataJSON();
        if (
          ["submit", "accept", "cancel", "stop", "resume"].includes(
            command?.kind,
          )
        )
          mutations.push({ path, command });
      });
    },
    // Fixture setup only: retains the existing admitted client/Origin/CSRF boundary.
    // Cookies and the private CSRF value stay in this browser context, never receipts.
    async admittedFixtureCommand(
      page: Page,
      command: QueueCommand,
    ): Promise<QueueRecord> {
      await expect.poll(() => !!queueAdmission).toBe(true);
      if (!queueAdmission)
        throw new Error("No observed admitted native queue port");
      const response = await page.request.post(queueAdmission.url, {
        headers: {
          origin,
          "x-lapis-protocol": "3",
          "x-lapis-csrf": queueAdmission.csrf,
        },
        data: command,
      });
      expect(response.ok()).toBe(true);
      admittedFixtureCommands.push(command);
      return (await response.json()).queue;
    },
    async login(page: Page) {
      await page.goto(origin);
      await page.getByLabel("Username").fill("owner");
      await page.getByLabel("Password").fill(password);
      await page.getByRole("button", { name: "Sign in", exact: true }).click();
      await page
        .getByRole("button", { name: "Open Rust vault", exact: true })
        .click();
      await page
        .getByRole("button", { name: "Open Chat", exact: true })
        .click();
    },
    async detach(page: Page) {
      await page.getByRole("button", { name: /Current workspace:/ }).click();
      await page
        .getByRole("menuitem", { name: "Manage Vaults", exact: true })
        .click();
      const released = page.waitForResponse(
        (r) =>
          r.request().method() === "DELETE" &&
          /\/clients\/[^/]+$/u.test(new URL(r.url()).pathname),
      );
      await page.getByRole("button", { name: "Sign out", exact: true }).click();
      expect((await released).status()).toBe(204);
    },
  };
}

/** Compare the documented public DTO without its retained private target guard. */
export function publicQueue(record: QueueRecord): QueueRecord {
  return {
    ...record,
    entries: record.entries.map((entry) => {
      const { target: _privateTarget, ...wire } = entry as typeof entry & {
        target?: unknown;
      };
      return wire;
    }),
  };
}

export async function queueAction(
  page: Page,
  kind: QueueCommand["kind"],
  action: () => Promise<void>,
): Promise<QueueRecord> {
  const response = page.waitForResponse(
    (r) =>
      r.url().endsWith("/queues") &&
      r.request().method() === "POST" &&
      r.request().postDataJSON()?.kind === kind,
  );
  await action();
  const result = await response;
  expect(result.ok()).toBe(true);
  return (await result.json()).queue;
}
