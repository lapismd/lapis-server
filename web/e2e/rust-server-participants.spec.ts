import { expect, test } from "@playwright/test";
import { readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { participantBrowser } from "./rust-participant-fixture";

test("declared logical roles share one runtime with independent output and passive retained-screen detach", async ({
  page,
}, testInfo) => {
  const f = await participantBrowser(page);
  await f.service.start();
  try {
    await f.open(true);
    await f.selectRoles();
    await f.send("One authored group café 📝", 2);
    await f.release("reviewer");
    await expect(f.chat).toContainText("held-reviewer:α:β:終");
    await expect(
      f.chat.getByRole("button", { name: "Stop agent", exact: true }),
    ).toBeVisible();
    await expect(
      f.chat.getByRole("button", { name: "Queue message", exact: true }),
    ).toHaveCount(0);
    const beforeDetach = await f.receipt();
    await expect
      .poll(async () =>
        readFile(join(f.service.vault, ".lapis/workspace.json"), "utf8").catch(
          (error: NodeJS.ErrnoException) => {
            if (error.code === "ENOENT") return "";
            throw error;
          },
        ),
      )
      .toContain(beforeDetach.conversation_id);
    await f.open();
    await expect(f.chat).toContainText("held-reviewer:α:β:終");
    await expect(
      f.chat.getByRole("button", { name: "Stop agent", exact: true }),
    ).toBeVisible();
    expect(
      (await f.frames()).filter((frame) => frame.type === "prompt"),
    ).toHaveLength(2);
    await f.release("writer");
    await expect(f.chat).toContainText("held-writer:α:β:終");
    await expect(
      f.chat.getByRole("button", { name: "Stop agent", exact: true }),
    ).toHaveCount(0);
    const message = await f.receipt();
    expect(
      message.input.profiles.map((p: any) => p.selection.recipient.id),
    ).toEqual(["writer", "reviewer"]);
    expect(message.input.turns.map((turn: any) => turn.target_id)).toEqual([
      "opaque-runtime",
      "opaque-runtime",
    ]);
    expect(
      new Set(
        message.deliveries.flatMap((delivery: any) => [
          delivery.request_id,
          delivery.agent_binding_id,
          delivery.session_id,
        ]),
      ).size,
    ).toBe(6);
    const directory = join(
      f.service.vault,
      ".lapis/agents/sessions",
      message.conversation_id,
    );
    const rows = (await readFile(join(directory, "transcript.jsonl"), "utf8"))
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line));
    expect(rows.filter((row) => row.role === "user")).toEqual([
      message.input.user,
    ]);
    const outputs = rows.filter((row) => row.role === "assistant");
    expect(outputs).toHaveLength(2);
    expect(outputs.map((row) => row.agentBindingId)).toEqual(
      message.deliveries.map((d: any) => d.agent_binding_id),
    );
    expect(JSON.stringify(rows)).not.toContain("never-in-portable-chat");
    expect(f.acceptances).toHaveLength(1);
    expect(f.mutations).toEqual([]);
    for (const captured of message.input.profiles) {
      expect(captured.selection.target.thinking).toBe("default");
      expect(captured.options.effort).toBeNull();
    }
    for (const frame of (await f.frames()).filter(
      (frame) => frame.type === "configure",
    ))
      expect(frame.effort).toBeNull();
    await f.service.restart();
    await f.open();
    await expect(f.chat).toContainText("held-writer:α:β:終");
    expect(
      (await f.frames()).filter((frame) => frame.type === "prompt"),
    ).toHaveLength(2);
  } catch (error) {
    await testInfo.attach("Rust participant host log", {
      body: f.service.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    await f.release();
    await f.service.stop();
  }
});

test("retained participant Stop and cold unknown observation preserve native authority without replay", async ({
  page,
}, testInfo) => {
  const f = await participantBrowser(page);
  await f.service.start();
  try {
    await f.open(true);
    await f.selectRoles();
    await f.send("Explicit native group Stop", 2);
    await f.chat
      .getByRole("button", { name: "Stop agent", exact: true })
      .click();
    await expect(
      f.chat.getByRole("button", { name: "Stop agent", exact: true }),
    ).toHaveCount(0);
    await expect
      .poll(async () =>
        (await f.receipt()).deliveries.every(
          (d: any) => !["pending", "started"].includes(d.state),
        ),
      )
      .toBe(true);
    await f.send("Cold participant checkpoint", 4);
    await expect
      .poll(async () =>
        (await f.receipt()).deliveries.every((d: any) => d.state === "started"),
      )
      .toBe(true);
    const restore = await f.checkpoint();
    await f.release();
    await expect(
      f.chat.getByRole("button", { name: "Stop agent", exact: true }),
    ).toHaveCount(0);
    await f.service.stop();
    await restore();
    await rm(f.log + ".release");
    f.declaration[0].participant_selections[0].recipient.label =
      "Renamed current writer";
    await writeFile(f.agents, JSON.stringify(f.declaration));
    await f.service.start();
    await f.open();
    await expect(f.chat).toContainText(
      "Outcome uncertain; it will not be replayed automatically",
    );
    await expect(f.chat).toContainText("Writer: Outcome uncertain");
    await expect(
      f.chat.getByRole("button", { name: "Stop agent", exact: true }),
    ).toHaveCount(0);
    const receipt = await f.receipt();
    expect(receipt.recovered).toBe(true);
    expect(receipt.deliveries.map((d: any) => d.state)).toEqual([
      "unknown",
      "unknown",
    ]);
    expect(receipt.input.profiles[0].selection.recipient.label).toBe("Writer");
    expect(
      (await f.frames()).filter((frame) => frame.type === "prompt"),
    ).toHaveLength(4);
    expect(f.acceptances).toHaveLength(2);
    expect(f.mutations).toEqual([]);
  } catch (error) {
    await testInfo.attach("Rust participant recovery host log", {
      body: f.service.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    await f.release();
    await f.service.stop();
  }
});
