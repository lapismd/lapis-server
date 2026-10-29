import { expect, test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { readFile, readdir, rename, writeFile } from "node:fs/promises";
import { join } from "node:path";
import type { TurnRecord } from "../../crates/lapis-app-protocol/bindings/protocol";
import {
  activeChat,
  composer,
  providerQueueFixture,
  publicQueue,
  queueAction,
} from "./rust-server-provider-queue.fixture";

test("installed provider resumes an offline recovery fixture only from its confirmed completed cold head", async ({
  browser,
}, testInfo) => {
  test.skip(process.env.LAPIS_SERVER_PROVIDER_SCREEN_TEST !== "1");
  const fixture = await providerQueueFixture();
  const { service, queue, turn, save } = fixture;
  const witness = `R4-RESUME-${randomUUID()}`;
  const expected = Array.from(
    { length: 32 },
    (_, i) => `${witness}:${String(i + 1).padStart(3, "0")}`,
  ).join("\n");
  const prompt = `Do not inspect files or use tools. Reply with exactly 32 lines and no other text. Each line must be ${witness}:NNN, replacing NNN with the consecutive zero-padded number from 001 through 032.`;
  const nextPrompt = `Return exactly ${witness}-QUEUED. Do not inspect files or use tools.`;
  const coldPrompt = `Return exactly ${witness}-RESUMED. Do not inspect files or use tools.`;
  let context = await browser.newContext({ serviceWorkers: "block" });
  try {
    await service.start();
    let page = await context.newPage();
    fixture.observe(page);
    await fixture.login(page);
    await composer(page).fill(prompt);
    const submitted = page.waitForResponse(
      (r) =>
        r.url().endsWith("/turns") &&
        r.request().method() === "POST" &&
        r.request().postDataJSON()?.kind === "submit",
    );
    await activeChat(page)
      .getByRole("button", { name: "Send message", exact: true })
      .click();
    const response = await submitted;
    expect(response.ok()).toBe(true);
    const accepted: TurnRecord = (await response.json()).record;
    const id = accepted.conversation_id,
      firstId = accepted.input.request_id;
    await expect.poll(async () => (await turn(firstId)).phase).toBe("running");
    await composer(page).fill(nextPrompt);
    const admitted = await queueAction(page, "accept", () =>
      activeChat(page)
        .getByRole("button", { name: "Queue message", exact: true })
        .click(),
    );
    expect(admitted.entries[0].phase).toBe("pending");
    await fixture.detach(page);
    expect((await turn(firstId)).phase).toBe("running");
    await context.close();
    await save("retained-composer-admission-and-live-detach", {
      firstId,
      queue: admitted,
    });
    await expect
      .poll(async () => (await queue(id)).entries[0].phase, {
        timeout: 150_000,
      })
      .toBe("completed");
    const first = await turn(firstId),
      secondId = admitted.entries[0].request_id;
    const second = await turn(secondId);
    expect(first.phase).toBe("completed");
    expect(first.output?.trim()).toBe(expected);
    expect(second.phase).toBe("completed");
    expect(second.output?.trim()).toBe(`${witness}-QUEUED`);
    const completed = await queue(id);
    expect(completed.head_request_id).toBe(secondId);
    expect(completed.entries[0].receipt_published).toBe(true);
    const beforeSetup = await fixture.transcriptHash(id);
    await save("confirmed-head-and-fifo-completed-without-browser", {
      first,
      second,
      queue: completed,
    });

    context = await browser.newContext({ serviceWorkers: "block" });
    page = await context.newPage();
    fixture.observe(page);
    await fixture.login(page);
    // Pause's product Stop also cancels active work. Prepare this fixture only
    // after both actual turns are confirmed complete; no uncertainty is resumed.
    const paused = await fixture.admittedFixtureCommand(page, {
      kind: "stop",
      conversation_id: id,
    });
    expect(paused.paused).toBe(true);
    expect(paused.barrier).toBe("explicit_stop");
    expect(paused.entries).toEqual(publicQueue(completed).entries);
    expect(paused.entries[0]).not.toHaveProperty("target");
    const input = structuredClone(admitted.entries[0].input);
    input.acceptance_id = randomUUID();
    input.after_request_id = secondId;
    const capture = input.capture as any;
    capture.id = randomUUID();
    capture.user.id = `queued-user-${randomUUID()}`;
    capture.user.text = coldPrompt;
    capture.user.createdAt = new Date().toISOString();
    // Offline recovery fixture only. Live Accept restarts an idle queue, so it
    // cannot represent this paused/unstarted state without a new product API.
    const seeded = structuredClone(await queue(id));
    const entry = structuredClone(seeded.entries[0]);
    entry.input = input;
    entry.request_id = randomUUID();
    entry.phase = "pending";
    entry.receipt = null;
    entry.receipt_published = false;
    seeded.entries.push(entry);
    expect(seeded.paused).toBe(true);
    expect(seeded.head_request_id).toBe(secondId);
    expect(seeded.entries[1].input).toEqual(input);
    expect(await turn(firstId)).toEqual(first);
    expect(await turn(secondId)).toEqual(second);
    expect(await fixture.transcriptHash(id)).toBe(beforeSetup);
    const journals = join(fixture.root, "profile/turns", service.vaultId);
    const journalNames = () =>
      readdir(journals).then((names) =>
        names.filter((n) => n.endsWith(".json")).sort(),
      );
    expect(await journalNames()).toEqual(
      [firstId, secondId].map((id) => `${id}.json`).sort(),
    );
    await context.close();
    await service.stop();
    // The ordinary host has exited cleanly. Preserve its real guards and all
    // existing entries; only this explicit fixture adds fresh pending identities.
    expect(publicQueue(await queue(id))).toEqual(paused);
    const queueFile = join(
      fixture.root,
      "profile/turns/queues",
      service.vaultId,
      `${id}.json`,
    );
    const bytes = await readFile(queueFile, "utf8");
    const temporary = `${queueFile}.fixture-${randomUUID()}`;
    await writeFile(temporary, JSON.stringify(seeded), {
      flag: "wx",
      mode: 0o600,
    });
    await rename(temporary, queueFile);
    expect(await queue(id)).toEqual(seeded);
    await save(
      "offline-recovery-fixture-after-confirmed-completion-no-dispatch",
      {
        setupKind: "offline-retained-queue-fixture",
        originalQueue: JSON.parse(bytes),
        fixtureQueue: seeded,
        transcriptSha256: beforeSetup,
      },
    );
    await service.start();
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
    expect(cold).toEqual({ ...seeded, barrier: "service_restarted" });
    expect(await journalNames()).toEqual(
      [firstId, secondId].map((id) => `${id}.json`).sort(),
    );
    expect(await fixture.transcriptHash(id)).toBe(beforeSetup);
    expect(await turn(firstId)).toEqual(first);
    expect(await turn(secondId)).toEqual(second);
    await save("cold-open-no-provider-dispatch-or-replay", {
      queue: cold,
      transcriptSha256: beforeSetup,
    });
    await queueAction(page, "resume", () => resume.click());
    await expect
      .poll(async () => (await queue(id)).entries[1].phase, {
        timeout: 150_000,
      })
      .toBe("completed");
    const final = await queue(id),
      resumedId = final.entries[1].request_id;
    const resumed = await turn(resumedId);
    expect(resumed.phase).toBe("completed");
    expect(resumed.output?.trim()).toBe(`${witness}-RESUMED`);
    await expect(
      activeChat(page).getByText(`${witness}-RESUMED`, { exact: true }),
    ).toHaveCount(1);
    expect(final.entries.map((e) => e.phase)).toEqual([
      "completed",
      "completed",
    ]);
    expect(final.entries[1].input).toEqual(input);
    expect(final.entries[1].receipt_published).toBe(true);
    expect(
      new Set([input.acceptance_id, resumedId, capture.id, capture.user.id])
        .size,
    ).toBe(4);
    for (const request of [first, second, resumed])
      expect(request.input.options).toMatchObject({
        model: null,
        effort: null,
        mode: "read-only",
      });
    const transcript = (await readFile(fixture.transcriptPath(id), "utf8"))
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line));
    expect(transcript.find((line) => line.id === capture.user.id)).toEqual(
      capture.user,
    );
    expect(
      transcript.find(
        (line) =>
          line.type === "queue.outcome" && line.queued.id === capture.id,
      ),
    ).toMatchObject({
      parentId: capture.user.id,
      outcome: "completed",
      queued: capture,
    });
    expect(fixture.mutations.map((m) => m.command.kind)).toEqual([
      "submit",
      "accept",
      "resume",
    ]);
    expect(fixture.mutations.at(-1)?.command).toEqual({
      kind: "resume",
      conversation_id: id,
      after_request_id: secondId,
    });
    expect(fixture.admittedFixtureCommands.map((c) => c.kind)).toEqual([
      "stop",
    ]);
    const finalHash = await fixture.transcriptHash(id);
    await page.reload();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await expect(
      activeChat(page).getByText(`${witness}-RESUMED`, { exact: true }),
    ).toHaveCount(1);
    expect(await queue(id)).toEqual(final);
    expect(await fixture.transcriptHash(id)).toBe(finalHash);
    expect(await journalNames()).toEqual(
      [firstId, secondId, resumedId].map((id) => `${id}.json`).sort(),
    );
    await save("retained-resume-exact-confirmed-head-defaults-receipt-reload", {
      queue: final,
      transcriptSha256: finalHash,
    });
  } catch (error) {
    await save("failed", { error: String(error) });
    await testInfo.attach("Rust provider resume host log", {
      body: service.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    await context.close();
    await service.stop();
  }
});
