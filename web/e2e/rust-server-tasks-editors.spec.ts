import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { EditingIntentState } from "@lapis-notes/api/native-editing-intents";
import { buildTasksArtifact } from "./tasks-installed-plugin";
import { createHostedTasksFixture, hostedTask } from "./rust-tasks-fixture";
import {
  moveCaretToEndOfDocument,
  openTaskSource,
} from "./tasks-editor-controls";

test.use({ locale: "en-GB", timezoneId: "Europe/London" });
let artifact: string;
test.beforeAll(async () => {
  artifact = await buildTasksArtifact(resolve("../.."));
});

test("another browser's real dirty editor fences Tasks and its owner flushes, yields and rearms", async ({
  page,
}, info) => {
  const f = await createHostedTasksFixture(page, artifact);
  const other = await page.context().newPage();
  let releaseSave!: () => void;
  const saveGate = new Promise<void>((resolve) => {
    releaseSave = resolve;
  });
  let heldSave = false;
  async function states(): Promise<EditingIntentState[]> {
    const reply = await f.execute({
      kind: "editing_intents",
      command: { kind: "list" },
    });
    expect(reply.outcome.status).toBe("ok");
    expect(reply.outcome.value).toMatchObject({
      kind: "editing_intents",
      value: { kind: "states" },
    });
    return reply.outcome.value.value.states.filter(
      (state: EditingIntentState) => state.path === "Hosted.md",
    );
  }
  try {
    await f.open();
    const owner = await f.openOther(other);
    const editor = await openTaskSource(other);
    let initial: EditingIntentState[] = [];
    await expect
      .poll(async () => {
        initial = await states();
        return initial.length;
      })
      .toBeGreaterThan(0);
    for (const state of initial)
      expect(state).toMatchObject({
        path: "Hosted.md",
        active: true,
        dirty: false,
        generation: 1,
      });
    await other.route("**/api/v3/clients/*/files/Hosted.md", async (route) => {
      if (route.request().method() === "PUT") {
        heldSave = true;
        await saveGate;
      }
      await route.continue();
    });
    await editor.click();
    await moveCaretToEndOfDocument(other);
    await other.keyboard.type("\nRetained real second-browser draft.\n");
    await expect(editor).toContainText("Retained real second-browser draft.");
    await expect
      .poll(async () => (await states()).some((state) => state.dirty))
      .toBe(true);
    await expect.poll(() => heldSave).toBe(true);
    await page.getByRole("button", { name: "Inbox", exact: true }).click();
    const refused = page.waitForResponse(
      (response) =>
        response.url().endsWith("/tasks") &&
        response.request().postDataJSON()?.kind === "submit",
    );
    await page
      .getByRole("checkbox", {
        name: "Mark Review the hosted migration complete",
        exact: true,
      })
      .first()
      .click();
    expect(await (await refused).json()).toMatchObject({
      kind: "failure",
      failure: { code: "BUSY", uncertain: false },
    });
    await expect(
      page.getByRole("alert").filter({ hasText: /editor|unresolved/i }),
    ).toBeVisible();
    expect(await readFile(join(f.service.vault, "Hosted.md"), "utf8")).toBe(
      hostedTask,
    );
    releaseSave();
    // The retained draft belongs at the document end; pin the flushed bytes
    // exactly so a caret that drifts into the raw frontmatter fails here
    // instead of silently invalidating the task document and emptying the
    // indexed task projection.
    const flushed = `${hostedTask}\nRetained real second-browser draft.\n`;
    await expect
      .poll(() => readFile(join(f.service.vault, "Hosted.md"), "utf8"))
      .toBe(flushed);
    await other
      .getByRole("button", { name: "Open Tasks", exact: true })
      .click();
    await other.getByRole("button", { name: "Inbox", exact: true }).click();
    const accepted = other.waitForResponse(
      (response) =>
        response.url().endsWith("/tasks") &&
        response.request().postDataJSON()?.kind === "submit",
    );
    await other
      .getByRole("checkbox", {
        name: "Mark Review the hosted migration complete",
        exact: true,
      })
      .first()
      .click();
    expect(await (await accepted).json()).toMatchObject({
      kind: "operation",
      operation: { disposition: "completed" },
    });
    await expect.poll(states).toEqual(
      expect.arrayContaining(
        initial.map((state) =>
          expect.objectContaining({
            editor_id: state.editor_id,
            active: true,
            dirty: false,
          }),
        ),
      ),
    );
    const rearmed = await states();
    for (const before of initial)
      expect(
        rearmed.find((state) => state.editor_id === before.editor_id)!
          .generation,
      ).toBeGreaterThan(before.generation);
    const saved = await readFile(join(f.service.vault, "Hosted.md"), "utf8");
    expect(saved).toContain("status: completed");
    expect(saved).toContain("Retained real second-browser draft.");
    expect(
      f.calls.filter(({ command }) => command.kind === "submit"),
    ).toHaveLength(2);
    expect(
      f.calls.filter(({ command }) => command.kind === "retry"),
    ).toHaveLength(0);
    expect(
      f.calls.some(
        ({ url, command }) =>
          url.includes(owner.client_id) && command.kind === "submit",
      ),
    ).toBe(true);
    const tabs = other.getByRole("toolbar", {
      name: "Workspace tabs",
      exact: true,
    });
    const hostedTab = tabs.getByRole("button", {
      name: "Hosted.md",
      exact: true,
    });
    await hostedTab.click();
    await hostedTab.hover();
    await tabs
      .getByRole("button", { name: "Close Hosted.md", exact: true })
      .click();
    await expect(hostedTab).toHaveCount(0);
    await expect(
      other.locator('.cm-content[contenteditable="true"]'),
    ).toHaveCount(0);
    await expect.poll(states).toEqual([]);
  } catch (error) {
    await info.attach("Editor assertion before teardown", {
      body: String(error),
      contentType: "text/plain",
    });
    await info.attach("Other editor page before teardown", {
      body: await other.locator("body").ariaSnapshot(),
      contentType: "text/yaml",
    });
    await info.attach("Native editor state before teardown", {
      body: JSON.stringify(await states()),
      contentType: "application/json",
    });
    await info.attach("Actual editor content before teardown", {
      body: JSON.stringify(
        await other.locator(".cm-content").allTextContents(),
      ),
      contentType: "application/json",
    });
    const problemCount = other.getByRole("button", { name: /^\d+ problems?$/ });
    if (await problemCount.isVisible()) {
      await problemCount.click();
      await info.attach("Close refusal Problems", {
        body: await other.locator("body").ariaSnapshot(),
        contentType: "text/yaml",
      });
    }
    throw error;
  } finally {
    releaseSave();
    await other.close();
    await f.stop(info);
  }
});
