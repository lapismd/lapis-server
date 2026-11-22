import { randomUUID } from "node:crypto";
import { expect, test } from "@playwright/test";
import { access, readFile, writeFile } from "node:fs/promises";
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
let tasksInstalled = false;
test.beforeAll(async () => {
  hostCode = await bundleExternalHost();
  tasksInstalled = await access(
    new URL(
      "../../.local/mcp-apps-dist/providers/tasks/tasks.html",
      import.meta.url,
    ),
  ).then(
    () => true,
    () => false,
  );
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
    await expect(view.locator("body")).toHaveCSS(
      "background-color",
      "rgb(255, 255, 255)",
    );
    await expect(view.getByText("private image")).toBeVisible();
    await expect(view.locator(".markdown").getByRole("link")).toHaveCount(0);
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

test("an older Tasks package keeps the MCP tool available", async () => {
  const f = await startMcpAppsFixture({ includeProviders: false });
  try {
    const { token } = await f.grant(["tasks:read"]);
    const { tools } = await f.rpc(token, "tools/list", {});
    const list = tools.find(
      (tool: { name: string }) => tool.name === "tasks_list",
    );
    expect(list).toBeDefined();
    expect(list._meta?.ui).toBeUndefined();
    const result = await f.rpc(token, "tools/call", {
      name: "tasks_list",
      arguments: { vault_id: f.server.vaultId },
    });
    expect(result.structuredContent.outcome.value.items).not.toHaveLength(0);
  } finally {
    await f.stop();
  }
});

test("edits and completes tasks with native receipts, including lost tracked completion responses", async ({
  page,
}) => {
  test.skip(!tasksInstalled, "The installed Tasks package has no MCP App.");
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
    await view
      .getByRole("button", {
        name: "Open task details for One-off task",
        exact: true,
      })
      .press("Enter");
    const detail = view.getByRole("region", { name: "Selected task" });
    await expect(
      detail.getByRole("textbox", { name: "Task title" }),
    ).toHaveText("One-off task");
    await writeFile(
      one,
      (await readFile(one, "utf8")) + "\nAn external edit.\n",
    );
    const scheduleTomorrow = async () => {
      await detail
        .getByRole("button", { name: "Set planner schedule", exact: true })
        .click();
      await view.getByRole("button", { name: "Tomorrow", exact: true }).click();
    };
    await scheduleTomorrow();
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
    const inbox = detail.getByRole("checkbox", { name: "inbox value" });
    await detail.getByRole("button", { name: "Expand properties" }).click();
    await inbox.click();
    await expect(view.getByRole("alert")).toContainText(
      "Reload the saved task",
    );
    expect(await readFile(one, "utf8")).toContain("inbox: false");
    expect(
      (await executeEditing({ kind: "discard", generation, revision: 1 }))
        .outcome.status,
    ).toBe("ok");
    await view.getByRole("button", { name: "Reload saved task" }).click();

    await scheduleTomorrow();
    await expect(view.getByText("Task updated.")).toBeVisible();
    const plan = await readFile(one, "utf8");
    expect(plan).toContain("date: 2026-08-21");
    expect(plan).toMatch(/at: ["']?09:00/u);
    expect(plan).toContain("30m");
    await view
      .getByRole("button", { name: "Close Task details column" })
      .click();
    await view
      .locator('[data-planner-task-id="one"]')
      .getByRole("button", { name: "Set planner schedule", exact: true })
      .click();
    await view.getByRole("button", { name: "Today", exact: true }).click();
    await expect
      .poll(() => readFile(one, "utf8"))
      .toContain("date: 2026-08-20");
    expect(await readFile(one, "utf8")).toContain("30m");
    // The saved file and the list projection can become visible at different times.
    await expect
      .poll(async () => {
        const result = await f.rpc(token, "tools/call", {
          name: "tasks_list",
          arguments: { vault_id: f.server.vaultId, limit: 50 },
        });
        return result.structuredContent.outcome.value.items.some(
          (item: { documentPath: string }) => item.documentPath === "One.md",
        );
      })
      .toBe(true);
    await view.getByRole("button", { name: "Refresh tasks" }).click();
    await view
      .getByRole("button", {
        name: "Open task details for One-off task",
        exact: true,
      })
      .click();
    await detail
      .getByRole("button", { name: "Clear planner schedule", exact: true })
      .click();
    await expect.poll(() => readFile(one, "utf8")).not.toContain("plan:");
    await detail.getByRole("button", { name: "Set due date" }).click();
    await view.getByRole("button", { name: "Tomorrow" }).click();
    await expect
      .poll(() => readFile(one, "utf8"))
      .toContain("deadline: 2026-08-21");
    await detail.getByRole("button", { name: "Set due date" }).click();
    await view.getByRole("button", { name: "Clear", exact: true }).click();
    await expect.poll(() => readFile(one, "utf8")).not.toContain("deadline:");
    await detail.getByRole("button", { name: "Expand properties" }).click();
    await inbox.click();
    await expect.poll(() => readFile(one, "utf8")).toContain("inbox: true");
    await inbox.click();
    await expect.poll(() => readFile(one, "utf8")).toContain("inbox: false");
    await detail.getByRole("checkbox", { name: "Mark task done" }).click();
    await expect(view.getByText("Task completed.")).toBeVisible();
    expect(await readFile(one, "utf8")).toContain("status: completed");
    await view
      .getByRole("button", { name: "Close Task details column" })
      .click();

    await view
      .getByRole("checkbox", { name: /^Mark Read.* complete$/ })
      .click();
    const dialog = view.getByRole("dialog", { name: /Complete Read/ });
    await dialog.getByRole("button", { name: "Cancel" }).click();
    expect(completions).toHaveLength(1);
    await view
      .getByRole("checkbox", { name: /^Mark Read.* complete$/ })
      .click();
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
  test.skip(!tasksInstalled, "The installed Tasks package has no MCP App.");
  const f = await startMcpAppsFixture();
  try {
    const { token } = await f.grant(["tasks:read"]);
    const view = await mountMcpApp(page, hostCode, f, token, "tasks_list", {
      vault_id: f.server.vaultId,
    });
    await view
      .getByRole("button", {
        name: "Open task details for One-off task",
        exact: true,
      })
      .click();
    const detail = view.getByRole("region", { name: "Selected task" });
    await expect(
      detail.getByRole("textbox", { name: "Task title" }),
    ).toHaveText("One-off task");
    await detail.getByRole("button", { name: "Expand properties" }).click();
    const inbox = detail.getByRole("checkbox", { name: "inbox value" });
    await expect(inbox).toBeVisible();
    await expect(inbox).toBeDisabled();
    await expect(inbox).not.toBeChecked();
    await expect(
      view.getByRole("checkbox", { name: "Mark One-off task complete" }),
    ).toBeDisabled();
    await expect(
      detail.getByRole("checkbox", { name: "Mark task done" }),
    ).toBeDisabled();
    const schedule = detail.locator(
      'button[aria-label="Set planner schedule"]',
    );
    await expect(schedule).toBeVisible();
    expect(
      await schedule.evaluate((button) => Boolean(button.closest("[inert]"))),
    ).toBe(true);
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
