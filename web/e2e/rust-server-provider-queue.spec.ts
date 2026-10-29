import { expect, test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import type { TurnRecord } from "../../crates/lapis-app-protocol/bindings/protocol";
import {
  activeChat,
  composer,
  providerQueueFixture,
  queueAction,
} from "./rust-server-provider-queue.fixture";

test("installed read-only provider retains exact Stop uncertainty and unstarted captures cold without replay", async ({
  browser,
}, testInfo) => {
  test.skip(process.env.LAPIS_SERVER_PROVIDER_SCREEN_TEST !== "1");
  const fixture = await providerQueueFixture();
  const { service, queue, turn, save } = fixture;
  const nonce = `R4-QUEUE-${randomUUID()}`;
  const expected = Array.from(
    { length: 48 },
    (_, i) => `${nonce}:${String(i + 1).padStart(3, "0")}`,
  ).join("\n");
  const firstPrompt = `Do not inspect files or use tools. Reply with exactly 48 lines and no other text. Each line must be ${nonce}:NNN, replacing NNN with the consecutive zero-padded number from 001 through 048.`;
  const cancelledPrompt = `Return exactly ${nonce}-NEVER. Do not inspect files or use tools.`;
  const stoppedPrompt = `Do not inspect files or use tools. Reply with exactly 80 lines and no other text. Each line must be ${nonce}-STOP:NNN, replacing NNN with the consecutive zero-padded number from 001 through 080.`;
  const resumedPrompt = `Return exactly ${nonce}-RESUMED. Do not inspect files or use tools.`;
  let context = await browser.newContext({ serviceWorkers: "block" });
  try {
    await service.start();
    let page = await context.newPage();
    fixture.observe(page);
    await fixture.login(page);
    await composer(page).fill(firstPrompt);
    const submitted = page.waitForResponse(
      (r) =>
        r.url().endsWith("/turns") &&
        r.request().method() === "POST" &&
        r.request().postDataJSON()?.kind === "submit",
    );
    await activeChat(page)
      .getByRole("button", { name: "Send message", exact: true })
      .click();
    const acceptedResponse = await submitted;
    expect(acceptedResponse.ok()).toBe(true);
    const accepted: TurnRecord = (await acceptedResponse.json()).record;
    const id = accepted.conversation_id;
    const firstId = accepted.input.request_id;
    expect(accepted.input.prompt).toBe(firstPrompt);
    expect(accepted.input.options).toMatchObject({
      model: null,
      effort: null,
      mode: "read-only",
    });
    await expect.poll(async () => (await turn(firstId)).phase).toBe("running");
    async function capture(prompt: string) {
      await composer(page).fill(prompt);
      return queueAction(page, "accept", () =>
        activeChat(page)
          .getByRole("button", { name: "Queue message", exact: true })
          .click(),
      );
    }
    const cancelled = await capture(cancelledPrompt);
    expect(cancelled.entries[0].phase).toBe("pending");
    const pending = activeChat(page).getByRole("region", {
      name: "Queued messages",
      exact: true,
    });
    await pending
      .getByRole("article", { name: "Pending message from You", exact: true })
      .hover();
    await queueAction(page, "cancel", () =>
      pending
        .getByRole("button", {
          name: "Cancel queued message for Codex",
          exact: true,
        })
        .click(),
    );
    expect((await queue(id)).entries[0].phase).toBe("cancelled");
    const second = await capture(stoppedPrompt);
    const third = await capture(resumedPrompt);
    const savedInputs = third.entries.map((entry) => entry.input);
    expect(second.entries[1].phase).toBe("pending");
    expect(third.entries[2].phase).toBe("pending");
    await save("actual-running-and-three-exact-captures", {
      firstId,
      conversationId: id,
      queue: third,
    });
    await fixture.detach(page);
    const detached = await turn(firstId);
    expect(detached.phase).toBe("running");
    await context.close();
    await save("live-detach-browser-closed", { phase: detached.phase });
    await expect
      .poll(
        async () =>
          !["accepted", "running"].includes((await turn(firstId)).phase),
        { timeout: 150_000 },
      )
      .toBe(true);
    const first = await turn(firstId);
    expect(first.phase).toBe("completed");
    expect(first.output?.trim()).toBe(expected);
    const secondId = third.entries[1].request_id;
    await expect
      .poll(async () => (await queue(id)).entries[1].phase, { timeout: 30_000 })
      .toBe("started");
    await save("first-completed-and-fifo-dispatched-without-browser", {
      first,
      queue: await queue(id),
    });

    context = await browser.newContext({ serviceWorkers: "block" });
    page = await context.newPage();
    fixture.observe(page);
    await fixture.login(page);
    await expect(
      activeChat(page).getByRole("button", {
        name: "Pause queue",
        exact: true,
      }),
    ).toBeVisible();
    await expect.poll(async () => (await turn(secondId)).phase).toBe("running");
    await queueAction(page, "stop", () =>
      activeChat(page)
        .getByRole("button", { name: "Pause queue", exact: true })
        .click(),
    );
    await expect
      .poll(async () => (await queue(id)).entries[1].phase, { timeout: 30_000 })
      .toBe("unknown");
    expect(await turn(secondId)).toMatchObject({
      phase: "unknown",
      failure_code: "vault_closed_during_turn",
    });
    const paused = await queue(id);
    expect(paused.paused).toBe(true);
    expect(paused.barrier).toBe("explicit_stop");
    expect(paused.entries[1].receipt_published).toBe(true);
    expect(paused.entries.map((entry) => entry.phase)).toEqual([
      "cancelled",
      "unknown",
      "pending",
    ]);
    expect(paused.entries.map((entry) => entry.input)).toEqual(savedInputs);
    expect(paused.head_request_id).toBe(secondId);
    const beforeCold = await fixture.transcriptHash(id);
    await save("explicit-stop-cleanup-with-unstarted-capture", {
      queue: paused,
      transcriptSha256: beforeCold,
    });
    await context.close();
    await service.restart();
    context = await browser.newContext({ serviceWorkers: "block" });
    page = await context.newPage();
    fixture.observe(page);
    await fixture.login(page);
    const resume = activeChat(page).getByRole("button", {
      name: "Resume queue",
      exact: true,
    });
    await expect(resume).toBeVisible();
    const cold = await queue(id);
    expect(cold.paused).toBe(true);
    expect(cold.head_request_id).toBe(secondId);
    expect(cold.entries).toEqual(paused.entries);
    expect(await turn(firstId)).toEqual(first);
    expect(await fixture.transcriptHash(id)).toBe(beforeCold);
    const journals = join(fixture.root, "profile/turns", service.vaultId);
    expect(
      (await readdir(journals)).filter((name) => name.endsWith(".json")).sort(),
    ).toEqual([firstId, secondId].map((id) => `${id}.json`).sort());
    await save("cold-paused-no-replay", {
      queue: cold,
      transcriptSha256: beforeCold,
    });
    const stopped = await turn(secondId);
    expect(stopped).toMatchObject({
      phase: "unknown",
      failure_code: "vault_closed_during_turn",
    });
    for (const entry of cold.entries) {
      const capture = entry.input.capture as any;
      expect(capture.selection).toEqual({
        targets: [{ id: "codex", agent: "codex", executionMode: "read-only" }],
        runtime: "acp",
        attachments: [],
      });
      expect(capture.user.recipients).toEqual([
        { id: "codex", label: "Codex", token: "jj-store-agent:636f646578;" },
      ]);
      expect(
        new Set([
          entry.input.acceptance_id,
          entry.request_id,
          capture.id,
          capture.user.id,
        ]).size,
      ).toBe(4);
    }
    expect(cold.entries[2].receipt).toBeNull();
    expect(cold.entries[2].receipt_published).toBe(false);
    for (const requestId of [firstId, secondId])
      expect((await turn(requestId)).input.options).toMatchObject({
        model: null,
        effort: null,
        mode: "read-only",
      });
    const transcript = (await readFile(fixture.transcriptPath(id), "utf8"))
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line));
    for (const entry of cold.entries.slice(0, 2)) {
      const capture = entry.input.capture as any;
      const authored = transcript.find((line) => line.id === capture.user.id);
      if (entry === cold.entries[0]) expect(authored).toBeUndefined();
      else expect(authored).toEqual(capture.user);
      expect(
        transcript.find(
          (line) =>
            line.type === "queue.outcome" && line.queued.id === capture.id,
        ),
      ).toEqual(entry.receipt);
      expect(entry.receipt).toMatchObject({
        parentId: capture.user.id,
        outcome: entry.phase,
        queued: capture,
      });
    }
    expect(fixture.mutations.map((entry) => entry.command.kind)).toEqual([
      "submit",
      "accept",
      "cancel",
      "accept",
      "accept",
      "stop",
    ]);
    await page.reload();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await expect(
      activeChat(page).getByRole("region", {
        name: "Queued messages",
        exact: true,
      }),
    ).toContainText(resumedPrompt);
    expect(await queue(id)).toEqual(cold);
    expect(await turn(firstId)).toEqual(first);
    expect(await turn(secondId)).toEqual(stopped);
    expect(await fixture.transcriptHash(id)).toBe(beforeCold);
    expect(
      (await readdir(journals)).filter((name) => name.endsWith(".json")).sort(),
    ).toEqual([firstId, secondId].map((id) => `${id}.json`).sort());
    expect(
      fixture.mutations.some((entry) => entry.command.kind === "resume"),
    ).toBe(false);
    await save("strict-stop-unknown-cold-reload-no-replay", {
      queue: cold,
      transcriptSha256: beforeCold,
    });
  } catch (error) {
    await save("failed", { error: String(error) });
    await testInfo.attach("Rust provider queue host log", {
      body: service.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    await context.close();
    await service.stop();
  }
});
