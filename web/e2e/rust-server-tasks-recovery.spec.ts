import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { buildTasksArtifact } from "./tasks-installed-plugin";
import { createHostedTasksFixture } from "./rust-tasks-fixture";
import { inspectRetainedTaskUpdate } from "./tasks-recovery-controls";

test.use({ locale: "en-GB", timezoneId: "Europe/London" });
let artifact: string;
test.beforeAll(async () => {
  artifact = await buildTasksArtifact(resolve("../.."));
});

test("another App saving ordinary settings preserves interrupted native Task references", async ({
  page,
  context,
}, info) => {
  const f = await createHostedTasksFixture(page, artifact);
  const other = await context.newPage();
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
    await f.open();
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
    await other.goto("about:blank");
    await other.close();
    await f.stop(info);
  }
});
