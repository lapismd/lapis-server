import { inspectRetainedTaskUpdate } from "./tasks-recovery-controls";
import { expect, test } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { buildTasksArtifact } from "./tasks-installed-plugin";
import { createHostedTasksFixture, hostedTask } from "./rust-tasks-fixture";

test.use({ locale: "en-GB", timezoneId: "Europe/London" });
let artifact: string;
test.beforeAll(async () => {
  artifact = await buildTasksArtifact(resolve("../.."));
});
const complete = (page: import("@playwright/test").Page, title: string) =>
  page
    .getByRole("checkbox", { name: `Mark ${title} complete`, exact: true })
    .first();
async function text(path: string): Promise<string> {
  return readFile(path, "utf8").catch((error: NodeJS.ErrnoException) => {
    if (error.code === "ENOENT") return "";
    throw error;
  });
}

test("installed Tasks planner admits native writes and reloads persisted state", async ({
  page,
}, info) => {
  const f = await createHostedTasksFixture(page, artifact);
  const taskPath = join(f.service.vault, "Hosted.md");
  try {
    await f.open();
    await page.getByRole("button", { name: "Inbox", exact: true }).click();
    await expect(
      page.getByText("Review the hosted migration", { exact: true }).first(),
    ).toBeVisible();
    expect(f.calls.map((call) => call.command)).toContainEqual(
      expect.objectContaining({
        kind: "configure",
        configuration: expect.objectContaining({
          format: { locale: "en-GB", firstDay: 1, minimalDays: 4 },
          provider: { kind: "default" },
        }),
      }),
    );
    await complete(page, "Review the hosted migration").click();
    await expect.poll(() => text(taskPath)).toContain("status: completed");
    expect(f.calls.map((call) => call.command)).toContainEqual(
      expect.objectContaining({ kind: "submit" }),
    );
    await f.open();
    await page.getByRole("button", { name: "Logbook", exact: true }).click();
    await expect(
      page.getByText("Review the hosted migration", { exact: true }).first(),
    ).toBeVisible();
    await f.restart();
    await f.open();
    await page.getByRole("button", { name: "Logbook", exact: true }).click();
    await expect(
      page.getByText("Review the hosted migration", { exact: true }).first(),
    ).toBeVisible();
    expect(await text(taskPath)).toContain("status: completed");
  } finally {
    await f.stop(info);
  }
});

test("matching operator and browser date materializes and completes the retained daily Task", async ({
  page,
}, info) => {
  const parts = Object.fromEntries(
    new Intl.DateTimeFormat("en-GB", {
      timeZone: "Europe/London",
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    })
      .formatToParts(new Date())
      .map((part) => [part.type, part.value]),
  );
  const today = `${parts.year}-${parts.month}-${parts.day}`;
  const corpus = JSON.parse(
    await readFile(
      resolve(
        "../../crates/lapis-app-core/tests/fixtures/task-tracked-today.json",
      ),
      "utf8",
    ),
  );
  const raw = corpus.cases[0].files["Habit.md"].replaceAll("2026-08-20", today);
  const f = await createHostedTasksFixture(page, artifact, { "Habit.md": raw });
  const daily = join(f.service.vault, `daily/${today}.md`);
  try {
    await f.open();
    expect(
      await page.evaluate(() => {
        const now = new Date();
        return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
      }),
    ).toBe(today);
    await expect
      .poll(
        async () => (await f.tasks({ kind: "status" })).status.tracked?.date,
      )
      .toBe(today);
    await expect
      .poll(() => text(daily))
      .toContain(`task=tracked date=${today} kind=boolean`);
    expect(await text(join(f.service.vault, "Habit.md"))).toBe(raw);
    await expect(complete(page, "A tracked habit")).toBeVisible();
    const submitted = page.waitForResponse(
      (response) =>
        response.url().endsWith("/tasks") &&
        response.request().method() === "POST" &&
        response.request().postDataJSON()?.kind === "submit",
    );
    await complete(page, "A tracked habit").click();
    const result = await (await submitted).json();
    expect(result.kind).toBe("operation");
    expect(result.operation.disposition).toBe("completed");
    expect(result.operation.daily_replacement).toEqual(expect.any(String));
    expect(await text(daily)).toBe(result.operation.daily_replacement);
    expect(await text(join(f.service.vault, "Habit.md"))).toBe(
      result.operation.task_replacement,
    );
    expect(await text(daily)).toContain("- [x] [A tracked habit]");
  } finally {
    await f.stop(info);
  }
});

test("stale source at native admission is refused visibly without replay", async ({
  page,
}, info) => {
  const f = await createHostedTasksFixture(page, artifact);
  const taskPath = join(f.service.vault, "Hosted.md");
  const external = hostedTask + "\nExternal content must survive.\n";
  let raced = false;
  try {
    await f.open();
    await page.getByRole("button", { name: "Inbox", exact: true }).click();
    await page.route("**/api/v3/clients/*/tasks", async (route) => {
      if (!raced && route.request().postDataJSON()?.kind === "submit") {
        raced = true;
        await writeFile(taskPath, external);
      }
      await route.continue();
    });
    const failed = page.waitForResponse(
      (response) =>
        response.url().endsWith("/tasks") &&
        response.request().method() === "POST" &&
        response.request().postDataJSON()?.kind === "submit",
    );
    await complete(page, "Review the hosted migration").click();
    const result = await (await failed).json();
    expect(result).toMatchObject({
      kind: "failure",
      failure: { code: "CONFLICT", uncertain: false },
    });
    await expect(
      page.getByRole("alert").filter({ hasText: /version|changed|conflict/i }),
    ).toBeVisible();
    expect(await text(taskPath)).toBe(external);
    expect(
      f.calls.filter(({ command }) => command.kind === "submit"),
    ).toHaveLength(1);
    expect(
      f.calls.filter(({ command }) => command.kind === "retry"),
    ).toHaveLength(0);
  } finally {
    await f.stop(info);
  }
});

test("lost response retains one accepted native operation and recovers on reload", async ({
  page,
}, info) => {
  const f = await createHostedTasksFixture(page, artifact);
  let accepted:
    | {
        kind: string;
        operation: { disposition: string; request: { request_id: string } };
      }
    | undefined;
  try {
    await f.open();
    await page.getByRole("button", { name: "Inbox", exact: true }).click();
    await page.route("**/api/v3/clients/*/tasks", async (route) => {
      if (route.request().postDataJSON()?.kind !== "submit")
        return route.continue();
      accepted = await (await route.fetch()).json();
      await route.abort("failed");
    });
    await complete(page, "Review the hosted migration").click();
    await expect(
      page.getByRole("alert").filter({ hasText: /unknown|inspect/i }),
    ).toBeVisible();
    expect(accepted).toMatchObject({
      kind: "operation",
      operation: { disposition: "completed" },
    });
    expect(await text(join(f.service.vault, "Hosted.md"))).toContain(
      "status: completed",
    );
    expect(
      f.calls.filter(({ command }) => command.kind === "submit"),
    ).toHaveLength(1);
    const requestId = accepted!.operation.request.request_id;
    const savedReferences = await f.references();
    expect(savedReferences).toEqual([
      expect.objectContaining({
        requestId,
        path: "Hosted.md",
        action: "complete",
      }),
    ]);
    expect(savedReferences[0]).not.toHaveProperty("task_replacement");
    expect(savedReferences[0]).not.toHaveProperty("original");

    await page.unroute("**/api/v3/clients/*/tasks");
    await f.open();
    await inspectRetainedTaskUpdate(page, requestId);
    await f.restart();
    await f.open();
    await inspectRetainedTaskUpdate(page, requestId);
    expect(
      f.calls.filter(
        ({ command }) =>
          command.kind === "inspect" && command.request_id === requestId,
      ),
    ).toHaveLength(2);
    await page.getByRole("button", { name: "Logbook", exact: true }).click();
    await expect(
      page.getByText("Review the hosted migration", { exact: true }).first(),
    ).toBeVisible();
    expect(
      f.calls.filter(({ command }) => command.kind === "submit"),
    ).toHaveLength(1);
    expect(
      f.calls.filter(({ command }) => command.kind === "retry"),
    ).toHaveLength(0);
  } finally {
    await f.stop(info);
  }
});

test("installed Tasks bytes remain hash-verified after the real plugin has loaded", async ({
  page,
}, info) => {
  const f = await createHostedTasksFixture(page, artifact);
  try {
    await f.open();
    const url = f.moduleUrl();
    expect(url).toBeTruthy();
    expect((await page.request.get(url!)).status()).toBe(200);
    const path = join(f.plugin, "main.mjs");
    const original = await readFile(path);
    try {
      await writeFile(
        path,
        Buffer.concat([original, Buffer.from("\n// changed\n")]),
      );
      expect((await page.request.get(url!)).status()).toBe(404);
    } finally {
      await writeFile(path, original);
    }
    expect((await page.request.get(url!)).status()).toBe(200);
    await f.open();
    await expect(
      page.getByRole("region", { name: "Task planner", exact: true }),
    ).toBeVisible();
  } finally {
    await f.stop(info);
  }
});

test("recoverable startup Retry reconfigures Tasks without detaching its App observer", async ({
  page,
}, info) => {
  const f = await createHostedTasksFixture(page, artifact);
  let refused = false;
  try {
    await page.route("**/api/v3/clients/*/tasks", async (route) => {
      if (!refused && route.request().postDataJSON()?.kind === "status") {
        refused = true;
        return route.fulfill({
          status: 503,
          json: { code: "IO", message: "Transient startup status unavailable" },
        });
      }
      await route.continue();
    });
    await f.open({ ready: false });
    const recovery = page
      .getByRole("alert")
      .filter({ hasText: "Lapis Notes could not start" });
    await expect(recovery).toBeVisible();
    await expect(recovery).toContainText(
      "Transient startup status unavailable",
    );
    const original = f.client();
    await recovery.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByText("Server DB", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Open Tasks", exact: true }).click();
    await expect(
      page.getByRole("region", { name: "Task planner", exact: true }),
    ).toBeVisible();
    expect(f.client()).toEqual(original);
    expect(
      f.calls.filter(({ command }) => command.kind === "configure"),
    ).toHaveLength(2);
    expect(await f.tasks({ kind: "status" })).toMatchObject({
      kind: "status",
      status: { enabled: true },
    });
    await page.getByRole("button", { name: "Inbox", exact: true }).click();
    await complete(page, "Review the hosted migration").click();
    await expect
      .poll(() => text(join(f.service.vault, "Hosted.md")))
      .toContain("status: completed");
  } finally {
    await f.stop(info);
  }
});

test("tampered installed Tasks never selects native scheduling and restored bytes load on fresh startup", async ({
  page,
}, info) => {
  const f = await createHostedTasksFixture(page, artifact);
  const path = join(f.plugin, "main.mjs");
  const original = await readFile(path);
  try {
    await writeFile(
      path,
      Buffer.concat([original, Buffer.from("\n// tampered before load\n")]),
    );
    await f.open({ tasks: false });
    await expect(
      page.getByRole("button", { name: "Open Tasks", exact: true }),
    ).toHaveCount(0);
    expect(
      f.calls.filter(({ command }) => command.kind === "configure"),
    ).toHaveLength(0);
    expect(await f.tasks({ kind: "status" })).toMatchObject({
      kind: "status",
      status: { enabled: false },
    });
    expect(f.pluginFailures).toHaveLength(1);
    expect(f.pluginFailures[0]).toMatch(
      /Failed to load plugin from \/\.lapis\/plugins\/tasks/,
    );
    expect(f.pluginFailures[0]).toMatch(/integrity|verified|fetch|module|404/i);
    await writeFile(path, original);
    expect(
      JSON.parse(
        await text(join(f.service.vault, ".lapis/community-plugins.json")),
      ),
    ).toContain("tasks");
    await f.open();
    await expect(
      page.getByRole("region", { name: "Task planner", exact: true }),
    ).toBeVisible();
    expect(await f.tasks({ kind: "status" })).toMatchObject({
      kind: "status",
      status: { enabled: true },
    });
  } finally {
    await writeFile(path, original);
    await f.stop(info, 1);
  }
});

test("switching vaults releases the observer without disabling native Tasks", async ({
  page,
}, info) => {
  const f = await createHostedTasksFixture(page, artifact);
  try {
    await f.open();
    const original = f.client();
    await page.getByRole("button", { name: /Current workspace:/ }).click();
    await page
      .getByRole("menuitem", { name: "Manage Vaults", exact: true })
      .click();
    await page.getByRole("button", { name: "Create New Vault" }).click();
    const dialog = page.getByRole("dialog", {
      name: "New Server Vault",
      exact: true,
    });
    await dialog.getByLabel("Name").fill("Other Tasks vault");
    await dialog
      .getByRole("button", { name: "Create vault", exact: true })
      .click();
    await expect(page.getByText("Server DB", { exact: true })).toBeVisible();
    expect(
      f.calls.filter(
        ({ url, command }) =>
          url.includes(`/clients/${original.client_id}/`) &&
          command.kind === "disable",
      ),
    ).toHaveLength(0);
    const headers = await f.headers();
    const response = await page.request.post(
      `${f.origin}/api/v3/vaults/${f.service.vaultId}/clients`,
      { headers, data: { protocol_version: 3 } },
    );
    expect(response.status()).toBe(200);
    const inspector = await response.json();
    try {
      const status = await page.request.post(
        `${f.origin}/api/v3/clients/${inspector.client_id}/tasks`,
        { headers, data: { kind: "status" } },
      );
      expect(status.ok()).toBe(true);
      expect(await status.json()).toMatchObject({
        kind: "status",
        status: { enabled: true },
      });
    } finally {
      expect(
        (
          await page.request.delete(
            `${f.origin}/api/v3/clients/${inspector.client_id}`,
            { headers },
          )
        ).status(),
      ).toBe(204);
    }
    expect(
      (
        await page.request.post(
          `${f.origin}/api/v3/clients/${original.client_id}/tasks`,
          { headers, data: { kind: "status" } },
        )
      ).status(),
    ).toBe(403);
    await f.open();
    await page.getByRole("button", { name: "Inbox", exact: true }).click();
    await expect(complete(page, "Review the hosted migration")).toBeVisible();
  } finally {
    await f.stop(info);
  }
});
