import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { EditingIntentState } from "@lapis-notes/api/native-editing-intents";
import { buildTasksArtifact } from "./tasks-installed-plugin";
import { createHostedTasksFixture, hostedTask } from "./rust-tasks-fixture";
import {
  manageVaults,
  moveCaretToEndOfDocument,
  openTaskSource,
  showEditingProblems,
} from "./tasks-editor-controls";

test.use({ locale: "en-GB", timezoneId: "Europe/London" });
let artifact: string;
test.beforeAll(async () => {
  artifact = await buildTasksArtifact(resolve("../.."));
});

for (const action of ["switch", "archive", "logout"] as const) {
  test(`guarded hosted leave preserves Problems after ${action} refusal and permits explicit retry`, async ({
    page,
    context,
  }, info) => {
    const f = await createHostedTasksFixture(page, artifact);
    const other = await context.newPage();
    let releaseSave!: () => void;
    const saveGate = new Promise<void>((resolve) => {
      releaseSave = resolve;
    });
    let heldSave = false;
    const shell = page.locator('[data-app-shell-ready="true"]');
    async function states(): Promise<EditingIntentState[]> {
      const reply = await f.execute({
        kind: "editing_intents",
        command: { kind: "list" },
      });
      expect(reply.outcome.status).toBe("ok");
      return reply.outcome.value.value.states.filter(
        (state: EditingIntentState) => state.path === "Hosted.md",
      );
    }
    async function leave() {
      if (action === "switch")
        await page
          .getByRole("button", { name: "Open Other Tasks vault", exact: true })
          .click();
      else if (action === "logout")
        await page
          .getByRole("button", { name: "Sign out", exact: true })
          .click();
      else {
        await page
          .getByRole("button", {
            name: "Open actions for Rust vault",
            exact: true,
          })
          .click();
        await page
          .getByRole("menuitem", { name: "Archive", exact: true })
          .click();
      }
    }
    try {
      await f.open();
      const original = f.client();
      if (action === "switch") {
        const created = await page.request.post(`${f.origin}/api/v3/vaults`, {
          headers: await f.headers(),
          data: { name: "Other Tasks vault" },
        });
        expect(created.status()).toBe(201);
      }
      const retainedShell = await shell.elementHandle();
      expect(retainedShell).not.toBeNull();
      const owner = await f.openOther(other);
      const oldVaultCalls = () =>
        f.calls.filter(({ url }) =>
          [original.client_id, owner.client_id].some((id) =>
            url.includes(`/clients/${id}/`),
          ),
        );
      const editor = await openTaskSource(other);
      await manageVaults(page);
      await other.route(
        "**/api/v3/clients/*/files/Hosted.md",
        async (route) => {
          if (route.request().method() === "PUT") {
            heldSave = true;
            await saveGate;
            if (other.isClosed()) return;
            return route.abort("failed");
          }
          await route.continue();
        },
      );
      await editor.click();
      await moveCaretToEndOfDocument(other);
      await other.keyboard.type("\nUnsaved lost browser draft.\n");
      await expect
        .poll(async () => (await states()).some((state) => state.dirty))
        .toBe(true);
      await expect.poll(() => heldSave).toBe(true);
      expect(await readFile(join(f.service.vault, "Hosted.md"), "utf8")).toBe(
        hostedTask,
      );
      expect(
        (
          await page.request.delete(
            `${f.origin}/api/v3/clients/${owner.client_id}`,
            {
              headers: await f.headers(),
            },
          )
        ).status(),
      ).toBe(204);
      // Revoke the lost browser first so its final unload cannot save the draft.
      await other.close();
      await expect
        .poll(async () =>
          (await states()).some((state) => state.detached && state.dirty),
        )
        .toBe(true);
      const lostClaims = new Set(
        (await states())
          .filter((state) => state.detached)
          .map((state) => state.editor_id),
      );
      expect(lostClaims.size).toBeGreaterThan(0);
      await leave();
      await expect(shell).toBeVisible();
      expect(
        await retainedShell!.evaluate((element) => element.isConnected),
      ).toBe(true);
      expect(await readFile(join(f.service.vault, "Hosted.md"), "utf8")).toBe(
        hostedTask,
      );
      await expect(
        page
          .getByRole("alert")
          .filter({ hasText: /Unresolved editing claims/ }),
      ).toBeVisible();
      const problem = await showEditingProblems(page);
      await expect(problem).toBeVisible();
      await problem.click({ button: "right" });
      await expect(
        page.getByRole("menuitem", { name: "Review saved file", exact: true }),
      ).toBeEnabled();
      await expect(
        page.getByRole("menuitem", {
          name: "Recover editing claim",
          exact: true,
        }),
      ).toBeEnabled();
      await page
        .getByRole("menuitem", { name: "Review saved file", exact: true })
        .click();
      await expect(
        page.getByRole("button", { name: /^Current view: preview/ }),
      ).toBeVisible();
      await expect(
        page.getByRole("tabpanel", { name: "Hosted.md", exact: true }),
      ).not.toContainText("Unsaved lost browser draft.");
      await expect(
        page
          .getByRole("tabpanel", { name: "Hosted.md", exact: true })
          .locator('.cm-content[contenteditable="true"]'),
      ).not.toBeVisible();
      await problem.click({ button: "right" });
      await page
        .getByRole("menuitem", { name: "Discard editing claim", exact: true })
        .click();
      await expect(problem).toHaveCount(0);
      await expect
        .poll(async () =>
          (await states()).filter(
            (state) => state.detached || lostClaims.has(state.editor_id),
          ),
        )
        .toEqual([]);
      expect(await readFile(join(f.service.vault, "Hosted.md"), "utf8")).toBe(
        hostedTask,
      );
      expect(await f.tasks({ kind: "status" })).toMatchObject({
        kind: "status",
        status: { enabled: true },
      });
      await page
        .getByRole("button", { name: "Open Tasks", exact: true })
        .click();
      await page.getByRole("button", { name: "Inbox", exact: true }).click();
      await expect(
        page
          .getByRole("checkbox", {
            name: "Mark Review the hosted migration complete",
            exact: true,
          })
          .first(),
      ).toBeVisible();
      expect(
        oldVaultCalls().filter(({ command }) => command.kind === "disable"),
      ).toHaveLength(0);
      await manageVaults(page);
      const detached = page.waitForResponse(
        (response) =>
          response.request().method() === "DELETE" &&
          response.url().endsWith(`/clients/${original.client_id}`),
      );
      await leave();
      expect((await detached).status()).toBe(204);
      if (action === "switch")
        await expect(
          page.getByText("Server DB", { exact: true }),
        ).toBeVisible();
      else if (action === "logout")
        await expect(
          page.getByRole("heading", { name: "Sign in", exact: true }),
        ).toBeVisible();
      else
        await expect(
          page.getByRole("button", {
            name: "Rust vault is archived",
            exact: true,
          }),
        ).toBeDisabled();
      expect(
        await retainedShell!.evaluate((element) => element.isConnected),
      ).toBe(false);
      expect(
        oldVaultCalls().filter(({ command }) => command.kind === "disable"),
      ).toHaveLength(0);
      expect(
        oldVaultCalls().filter(({ command }) => command.kind === "retry"),
      ).toHaveLength(0);
    } catch (error) {
      await info.attach("Guarded leave assertion before teardown", {
        body: String(error),
        contentType: "text/plain",
      });
      await info.attach("Guarded leave page before teardown", {
        body: await page.locator("body").ariaSnapshot(),
        contentType: "text/yaml",
      });
      if (!other.isClosed())
        await info.attach("Lost browser page before teardown", {
          body: await other.locator("body").ariaSnapshot(),
          contentType: "text/yaml",
        });
      throw error;
    } finally {
      releaseSave();
      if (!other.isClosed()) await other.close();
      await f.stop(info);
    }
  });
}

for (const outcome of ["pending", "failed"] as const) {
  test(`guarded hosted leave retains a real ${outcome} writer until its draft saves`, async ({
    page,
  }, info) => {
    const f = await createHostedTasksFixture(page, artifact);
    let releaseSave!: () => void;
    const saveGate = new Promise<void>((resolve) => {
      releaseSave = resolve;
    });
    let writes = 0;
    const draft = `${hostedTask}\nPersist the local draft before leaving.\n`;
    try {
      await f.open();
      const editor = await openTaskSource(page);
      let detached = false;
      const original = f.client();
      page.on("request", (request) => {
        if (
          request.method() === "DELETE" &&
          request.url().endsWith(`/clients/${original.client_id}`)
        )
          detached = true;
      });
      const shell = await page
        .locator('[data-app-shell-ready="true"]')
        .elementHandle();
      await page.route("**/api/v3/clients/*/files/Hosted.md", async (route) => {
        if (route.request().method() !== "PUT") return route.continue();
        writes++;
        if (outcome === "failed") return route.abort("connectionrefused");
        await saveGate;
        await route.continue();
      });
      await editor.fill(draft);
      await expect.poll(() => writes).toBeGreaterThan(0);
      await manageVaults(page);
      await page.getByRole("button", { name: "Sign out", exact: true }).click();
      if (outcome === "pending") {
        // Preflight drains accepted writes before taking the final writer guard.
        await page
          .getByRole("button", { name: "Return to Rust vault", exact: true })
          .click();
        await expect(
          page.locator('[data-app-shell-ready="true"]'),
        ).toBeVisible();
        await expect(editor).toContainText(
          "Persist the local draft before leaving.",
        );
        expect(detached).toBe(false);
        expect(await shell!.evaluate((element) => element.isConnected)).toBe(
          true,
        );
        await expect(
          page.getByRole("heading", { name: "Sign in", exact: true }),
        ).toHaveCount(0);
        expect(await readFile(join(f.service.vault, "Hosted.md"), "utf8")).toBe(
          hostedTask,
        );
        releaseSave();
      } else {
        await expect(
          page.locator('[data-app-shell-ready="true"]'),
        ).toBeVisible();
        await expect(
          page.getByRole("alert").filter({ hasText: "Vault could not close" }),
        ).toBeVisible();
        expect(await shell!.evaluate((element) => element.isConnected)).toBe(
          true,
        );
        await expect(editor).toContainText(
          "Persist the local draft before leaving.",
        );
        expect(await readFile(join(f.service.vault, "Hosted.md"), "utf8")).toBe(
          hostedTask,
        );
        await page.unroute("**/api/v3/clients/*/files/Hosted.md");
        await manageVaults(page);
        await page
          .getByRole("button", { name: "Sign out", exact: true })
          .click();
      }
      await expect(
        page.getByRole("heading", { name: "Sign in", exact: true }),
      ).toBeVisible();
      expect(await readFile(join(f.service.vault, "Hosted.md"), "utf8")).toBe(
        draft,
      );
      expect(await shell!.evaluate((element) => element.isConnected)).toBe(
        false,
      );
      expect(
        f.calls.filter(({ command }) => command.kind === "disable"),
      ).toHaveLength(0);
      expect(
        f.calls.filter(({ command }) => command.kind === "retry"),
      ).toHaveLength(0);
    } catch (error) {
      await info.attach("Pending writer leave assertion", {
        body: String(error),
        contentType: "text/plain",
      });
      await info.attach("Pending writer leave page", {
        body: await page.locator("body").ariaSnapshot(),
        contentType: "text/yaml",
      });
      throw error;
    } finally {
      releaseSave();
      await f.stop(info);
    }
  });
}
