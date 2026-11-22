import { randomUUID } from "node:crypto";
import { expect, test } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  bundleExternalHost,
  mountMcpApp,
  startMcpAppsFixture,
} from "./mcp-apps-fixture";

test.use({
  timezoneId: "America/Los_Angeles",
  viewport: { width: 380, height: 740 },
});

let hostCode: string;
test.beforeAll(async () => {
  hostCode = await bundleExternalHost();
});

test("serves the Notes view only through MCP and refreshes saved Markdown inside the sandbox", async ({
  page,
}) => {
  const f = await startMcpAppsFixture();
  try {
    const { token } = await f.grant([]);
    expect((await fetch(f.origin + "/notes.html")).status).toBe(404);
    const requests: string[] = [];
    page.on("request", (request) => requests.push(request.url()));
    const view = await mountMcpApp(page, hostCode, f, token, "read_note", {
      vault_id: f.server.vaultId,
      path: "Note.md",
    });
    await expect(
      view.getByRole("heading", { name: "Saved note" }),
    ).toBeVisible();
    await expect(view.locator("strong")).toHaveText("Important");
    await expect(view.getByText("private image")).toBeVisible();
    await expect(view.getByRole("link")).toHaveCount(0);
    expect(requests.filter((url) => url.includes("example.invalid"))).toEqual(
      [],
    );
    await page.evaluate(() => localStorage.setItem("host-private", "secret"));
    expect(
      await view.locator("body").evaluate(async () => {
        let hostRead = false;
        let escaped = false;
        try {
          hostRead = Boolean(window.top?.localStorage.getItem("host-private"));
        } catch {}
        try {
          window.top!.location.href = "about:blank";
          escaped = true;
        } catch {}
        const network = await fetch("https://example.invalid/request").then(
          () => true,
          () => false,
        );
        return { hostRead, escaped, network };
      }),
    ).toEqual({ hostRead: false, escaped: false, network: false });
    expect(page.url()).toBe("http://127.0.0.1:58191/");
    await writeFile(join(f.server.vault, "Note.md"), "# Saved revision");
    await view.getByRole("button", { name: "Refresh" }).press("Enter");
    await expect(
      view.getByRole("heading", { name: "Saved revision" }),
    ).toBeVisible();
  } finally {
    await f.stop();
  }
});

test("edits and completes tasks with native receipts, including lost tracked completion responses", async ({
  page,
}) => {
  const f = await startMcpAppsFixture();
  try {
    await page.clock.setFixedTime(new Date("2026-08-21T02:00:00Z"));
    const { token } = await f.grant(["tasks:read", "tasks:write"]);
    const completions: Record<string, unknown>[] = [];
    const view = await mountMcpApp(
      page,
      hostCode,
      f,
      token,
      "tasks_list",
      {
        vault_id: f.server.vaultId,
        limit: 50,
      },
      (name, args) => {
        if (name !== "tasks_complete") return false;
        completions.push(args);
        return args.path === "Read.md";
      },
    );
    const one = join(f.server.vault, "One.md");
    await view.getByRole("button", { name: "One-off task" }).press("Enter");
    const detail = view.getByRole("region", { name: "Selected task" });
    await expect(detail.getByRole("heading")).toHaveText("One-off task");
    await writeFile(
      one,
      (await readFile(one, "utf8")) + "\nAn external edit.\n",
    );
    await detail.getByLabel("Planned date").fill("2026-08-21");
    await expect(view.getByRole("alert")).toContainText(
      "Reload the saved task",
    );
    expect(await readFile(one, "utf8")).toContain("date: 2026-08-20");
    await view.getByRole("button", { name: "Reload saved task" }).click();

    const editorId = randomUUID();
    const executeEditing = (command: Record<string, unknown>) =>
      f.command({
        kind: "editing_intents",
        command: { ...command, editor_id: editorId },
      });
    const acquired = await executeEditing({ kind: "acquire", path: "One.md" });
    const generation = acquired.outcome.value.value.state.generation;
    expect(
      (await executeEditing({ kind: "changed", generation, revision: 1 }))
        .outcome.status,
    ).toBe("ok");
    await detail.getByLabel("In Inbox").click();
    await expect(view.getByRole("alert")).toContainText(
      "Reload the saved task",
    );
    expect(await readFile(one, "utf8")).toContain("inbox: false");
    expect(
      (await executeEditing({ kind: "discard", generation, revision: 1 }))
        .outcome.status,
    ).toBe("ok");
    await view.getByRole("button", { name: "Reload saved task" }).click();

    await detail.getByLabel("Planned date").fill("2026-08-21");
    await expect(view.getByText("Task updated.")).toBeVisible();
    const plan = await readFile(one, "utf8");
    expect(plan).toContain("date: 2026-08-21");
    expect(plan).toMatch(/at: ["']?09:00/u);
    expect(plan).toContain("30m");
    await detail.getByRole("button", { name: "Clear planned date" }).click();
    await expect.poll(() => readFile(one, "utf8")).not.toContain("plan:");
    await detail.getByRole("button", { name: "Set due date" }).click();
    await view.getByRole("button", { name: "Tomorrow" }).click();
    await expect
      .poll(() => readFile(one, "utf8"))
      .toContain("deadline: 2026-08-21");
    await detail.getByRole("button", { name: "Set due date" }).click();
    await view.getByRole("button", { name: "Clear", exact: true }).click();
    await expect.poll(() => readFile(one, "utf8")).not.toContain("deadline:");
    await detail.getByLabel("In Inbox").click();
    await expect.poll(() => readFile(one, "utf8")).toContain("inbox: true");
    await detail.getByLabel("In Inbox").click();
    await expect.poll(() => readFile(one, "utf8")).toContain("inbox: false");
    await detail.getByRole("button", { name: "Complete", exact: true }).click();
    await expect(view.getByText("Task completed.")).toBeVisible();
    expect(await readFile(one, "utf8")).toContain("status: completed");

    await view.getByRole("button", { name: /Read/ }).click();
    await detail.getByRole("button", { name: "Complete", exact: true }).click();
    const dialog = view.getByRole("dialog", { name: /Complete Read/ });
    await dialog.getByRole("button", { name: "Cancel" }).click();
    expect(completions).toHaveLength(1);
    await detail.getByRole("button", { name: "Complete", exact: true }).click();
    await dialog.getByLabel("Duration").fill("8m");
    await dialog.getByRole("button", { name: "Complete", exact: true }).click();
    await expect(view.getByRole("alert")).toContainText(
      "Completion could not be confirmed",
    );
    const requestId = completions[1].invocation_id;
    expect(completions[1]).toMatchObject({
      localDate: "2026-08-20",
      trackedValue: 8,
    });
    await expect(view.getByText(String(requestId))).toBeVisible();
    await view.getByRole("button", { name: "Check completion status" }).click();
    await expect(view.getByText("Task completed.")).toBeVisible();
    expect(completions).toHaveLength(2);
    expect(await readFile(join(f.server.vault, "Read.md"), "utf8")).toContain(
      "date: 2026-08-21",
    );
    const daily = await readFile(
      join(f.server.vault, "daily/2026-08-20.md"),
      "utf8",
    );
    expect(daily).toContain(
      "- [x] [Read 📝](../Read.md) : 8m <!-- task-occurrence task=read date=2026-08-20 kind=duration -->",
    );
    expect(daily.match(/<!-- task-occurrence task=read /gu)).toHaveLength(1);
  } finally {
    await f.stop();
  }
});

test("read grants show task details without write controls", async ({
  page,
}) => {
  const f = await startMcpAppsFixture();
  try {
    const { token } = await f.grant(["tasks:read"]);
    const view = await mountMcpApp(page, hostCode, f, token, "tasks_list", {
      vault_id: f.server.vaultId,
    });
    await view.getByRole("button", { name: "One-off task" }).click();
    const detail = view.getByRole("region", { name: "Selected task" });
    await expect(detail.getByRole("heading")).toHaveText("One-off task");
    await expect(detail.getByText("Outside Inbox")).toBeVisible();
    await expect(detail.getByRole("button")).toHaveCount(0);
    expect(await detail.locator("input").count()).toBe(0);
    expect(
      await view
        .locator("html")
        .evaluate((root) => root.scrollWidth <= root.clientWidth),
    ).toBe(true);

    await f.send(f.client + "/tasks", { kind: "disable" });
    const { tools } = await f.rpc(token, "tools/list", {});
    expect(
      tools.some((tool: { name: string }) => tool.name === "tasks_list"),
    ).toBe(false);
    await expect(
      f.rpc(token, "resources/read", { uri: "ui://lapis/tasks/task-list-v1" }),
    ).rejects.toThrow("MCP error");
  } finally {
    await f.stop();
  }
});
