import { expect, test, type Page } from "@playwright/test";
import { createHash, randomBytes, randomUUID } from "node:crypto";
import { mkdir, mkdtemp, readFile, readdir, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { TurnRecord } from "../../crates/lapis-app-protocol/bindings/protocol";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";

test("actual provider finishes after retained chat signs out and survives a cold service restart", async ({
  browser,
}, testInfo) => {
  test.skip(process.env.LAPIS_SERVER_PROVIDER_SCREEN_TEST !== "1");
  const configuration = process.env.LAPIS_SERVER_TEST_AGENTS_CONFIG;
  if (!configuration)
    throw new Error("Set LAPIS_SERVER_TEST_AGENTS_CONFIG for host acceptance");
  const repository = resolve("../..");
  const root = await mkdtemp(join(repository, ".local/rust-provider-screen-"));
  const agents = join(root, "agents.json");
  await writeFile(agents, await readFile(resolve(configuration)), {
    mode: 0o600,
  });
  const password = randomBytes(32).toString("base64url");
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const service = new RustServerRuntime(
    root,
    repository,
    origin,
    password,
    agents,
  );
  const witness = `R2-SCREEN-${randomUUID()}`;
  const prompt = `Return exactly the string ${witness}. Do not inspect files or use tools.`;
  const stages: Array<Record<string, unknown>> = [];
  const submittedRequests: string[] = [];
  function observe(page: Page) {
    page.on("request", (request) => {
      if (request.method() !== "POST" || !request.url().endsWith("/turns"))
        return;
      const command = request.postDataJSON();
      if (command?.kind === "submit")
        submittedRequests.push(command.input.request_id);
    });
  }
  const activeChat = (page: Page) =>
    page.locator('[data-native-chat="true"]:visible');
  async function save(stage: string, facts: Record<string, unknown> = {}) {
    stages.push({ stage, at: new Date().toISOString(), ...facts });
    await writeFile(
      join(root, "receipt.json"),
      JSON.stringify({ root, witness, stages }, null, 2),
    );
  }
  async function login(page: Page) {
    await page.goto(origin);
    await page.getByLabel("Username").fill("owner");
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
  }
  async function openHistory(page: Page) {
    const chat = activeChat(page);
    await chat
      .getByRole("button", { name: "Conversation actions", exact: true })
      .click();
    await page
      .getByRole("menuitem", { name: "AI conversations", exact: true })
      .click();
    return page.getByTestId("ai-conversation-history");
  }
  await mkdir(join(service.vault, ".lapis"), { recursive: true });
  await writeFile(
    join(service.vault, ".lapis/core-plugins.json"),
    JSON.stringify({ disabled: [], enabled: ["ai"] }),
  );
  let context = await browser.newContext({ serviceWorkers: "block" });
  try {
    await service.start();
    let page = await context.newPage();
    observe(page);
    await login(page);
    await page.getByRole("button", { name: "Open Chat", exact: true }).click();
    const chat = activeChat(page);
    await chat
      .getByRole("textbox", { name: "Message", exact: true })
      .fill(prompt);
    const submitted = page.waitForResponse((response) => {
      if (
        !response.url().endsWith("/turns") ||
        response.request().method() !== "POST"
      )
        return false;
      return response.request().postDataJSON()?.kind === "submit";
    });
    await chat
      .getByRole("button", { name: "Send message", exact: true })
      .click();
    const response = await submitted;
    expect(response.ok()).toBe(true);
    const accepted: TurnRecord = (await response.json()).record;
    expect(["accepted", "running"]).toContain(accepted.phase);
    expect(accepted.input.prompt).toBe(prompt);
    expect(accepted.input.options.mode).toBe("read-only");
    expect(accepted.input.options.model).toBeNull();
    const journal = join(
      root,
      "profile/turns",
      service.vaultId,
      `${accepted.input.request_id}.json`,
    );
    const readRecord = async (): Promise<TurnRecord> =>
      JSON.parse(await readFile(journal, "utf8"));
    await save("accepted-through-composer", {
      requestId: accepted.input.request_id,
      conversationId: accepted.conversation_id,
      phase: accepted.phase,
    });
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
    await expect(
      page.getByRole("button", { name: "Sign in", exact: true }),
    ).toBeVisible();
    const detached = await readRecord();
    expect(["accepted", "running"]).toContain(detached.phase);
    await context.close();
    await save("detached-without-browser", { phase: detached.phase });
    await expect
      .poll(async () => (await readRecord()).phase, { timeout: 150_000 })
      .toBe("completed");
    const completed = await readRecord();
    expect(completed.output?.trim()).toBe(witness);
    const transcript = join(
      service.vault,
      ".lapis/agents/sessions",
      completed.conversation_id,
      "transcript.jsonl",
    );
    const hash = async () =>
      createHash("sha256")
        .update(await readFile(transcript))
        .digest("hex");
    const transcriptSha256 = await hash();
    await save("completed-without-browser", {
      phase: completed.phase,
      transcriptSha256,
    });
    await service.restart();
    context = await browser.newContext({ serviceWorkers: "block" });
    page = await context.newPage();
    observe(page);
    await login(page);
    await page.getByRole("button", { name: "Open Chat", exact: true }).click();
    const history = await openHistory(page);
    await expect(history).toBeVisible();
    await history
      .getByRole("button", {
        name: new RegExp(`^Return exactly the string ${witness}`),
      })
      .click();
    await expect(
      activeChat(page).getByText(witness, { exact: true }),
    ).toHaveCount(1);
    expect(await readRecord()).toEqual(completed);
    expect(await hash()).toBe(transcriptSha256);
    const turns = page.waitForResponse(
      (r) =>
        r.url().endsWith("/turns") &&
        r.request().postDataJSON()?.kind === "list",
    );
    await page.reload();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await turns;
    await expect(
      activeChat(page).getByText(witness, { exact: true }),
    ).toHaveCount(1);
    expect(await readRecord()).toEqual(completed);
    expect(await hash()).toBe(transcriptSha256);
    expect(submittedRequests.map((id) => id.replaceAll("-", ""))).toEqual([
      accepted.input.request_id.replaceAll("-", ""),
    ]);
    expect(
      (await readdir(join(root, "profile/turns", service.vaultId))).filter(
        (name) => name.endsWith(".json"),
      ),
    ).toEqual([`${accepted.input.request_id}.json`]);
    await save("cold-history-and-reload-without-replay", {
      transcriptSha256,
      submittedRequests,
    });
  } catch (error) {
    await save("failed", { error: String(error) });
    await testInfo.attach("Rust provider host log", {
      body: service.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    await context.close();
    await service.stop();
  }
});
