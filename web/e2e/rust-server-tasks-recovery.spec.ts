import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { buildTasksArtifact } from "./tasks-installed-plugin";
import { createHostedTasksFixture } from "./rust-tasks-fixture";
import { inspectRetainedTaskUpdate } from "./tasks-recovery-controls";
import { signOutHostedWorkspace } from "./rust-hosted-session";

test.use({ locale: "en-GB", timezoneId: "Europe/London" });
let artifact: string;
test.beforeAll(async () => {
  artifact = await buildTasksArtifact(resolve(import.meta.dirname, "../.."));
});

test("another App saving ordinary settings preserves interrupted native Task references", async ({
  page,
  browser,
}, info) => {
  const f = await createHostedTasksFixture(page, artifact);
  // Independent account sessions let each App await guarded logout without
  // revoking the other App's authentication while it still owns observers.
  const otherContext = await browser.newContext({
    locale: "en-GB",
    timezoneId: "Europe/London",
  });
  const other = await otherContext.newPage();
  let requestId: string | undefined;
  try {
    await f.open();
    await f.openOther(other);
    await page.getByRole("button", { name: "Inbox", exact: true }).click();
    await page.route("**/api/v3/clients/*/tasks", async (route) => {
      if (route.request().postDataJSON()?.kind !== "submit")
        return route.continue();
      const accepted = await (await route.fetch()).json();
      expect(accepted).toMatchObject({
        kind: "operation",
        operation: { disposition: "completed" },
      });
      requestId = accepted.operation.request.request_id;
      await route.abort("failed");
    });
    await page
      .getByRole("checkbox", {
        name: "Mark Review the hosted migration complete",
        exact: true,
      })
      .first()
      .click();
    await expect(
      page.getByRole("alert").filter({ hasText: /unknown|inspect/i }),
    ).toBeVisible();
    expect(requestId).toEqual(expect.any(String));
    await other
      .getByRole("button", { name: "Collapse Tasks column", exact: true })
      .click();
    await expect
      .poll(async () => {
        const value = await readFile(join(f.plugin, "data.json"), "utf8").catch(
          (error: NodeJS.ErrnoException) => {
            if (error.code === "ENOENT") return "null";
            throw error;
          },
        );
        return JSON.parse(value);
      })
      .toMatchObject({
        settings: { planner: { navigationColumnState: "collapsed" } },
      });
    await page.unroute("**/api/v3/clients/*/tasks");
    const previousClient = f.client().client_id;
    const lifetime: string[] = [];
    page.on("response", (response) => {
      if (
        response.url() === `${f.origin}/api/v3/clients/${previousClient}` &&
        response.request().method() === "DELETE" &&
        response.ok()
      )
        lifetime.push("released");
      if (
        response.url().endsWith(`/vaults/${f.service.vaultId}/clients`) &&
        response.request().method() === "POST" &&
        response.ok()
      )
        lifetime.push("admitted");
    });
    await f.open();
    expect(lifetime).toEqual(["released", "admitted"]);
    await inspectRetainedTaskUpdate(page, requestId!);
    expect(
      f.calls.filter(({ command }) => command.kind === "submit"),
    ).toHaveLength(1);
    expect(
      f.calls.filter(({ command }) => command.kind === "retry"),
    ).toHaveLength(0);
    expect(
      await readFile(join(f.service.vault, "Hosted.md"), "utf8"),
    ).toContain("status: completed");
  } catch (error) {
    await info.attach("Concurrent Tasks page before teardown", {
      body: await page.locator("body").ariaSnapshot(),
      contentType: "text/yaml",
    });
    await info.attach("Concurrent Tasks calls", {
      body: JSON.stringify(f.calls),
      contentType: "application/json",
    });
    await info.attach("Concurrent Tasks native log", {
      body: f.service.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    // Both Apps leave through their own session's guarded logout boundary.
    try {
      if (!other.isClosed()) {
        await signOutHostedWorkspace(other);
        await other.goto("about:blank");
      }
    } finally {
      try {
        await otherContext.close();
      } finally {
        await f.stop(info);
      }
    }
  }
});
