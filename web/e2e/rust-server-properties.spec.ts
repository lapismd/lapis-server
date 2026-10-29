import { expect, test, type Locator, type Page } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { buildTasksArtifact } from "./tasks-installed-plugin";
import { createHostedTasksFixture } from "./rust-tasks-fixture";
import { manageVaults, openTaskSource } from "./tasks-editor-controls";

const original = "---\nstatus: ready\n---\n# Hosted Properties\n";
let artifact: string;
test.use({ locale: "en-GB", timezoneId: "Europe/London" });
test.beforeAll(async () => {
  artifact = await buildTasksArtifact(resolve("../.."));
});

async function properties(page: Page) {
  const source = await openTaskSource(page);
  await page.getByRole("button", { name: "Open command palette", exact: true }).click();
  const palette = page.getByRole("dialog", { name: "Command Palette", exact: true });
  await palette.getByRole("tab", { name: "Actions", exact: true }).click();
  await palette.getByRole("combobox", { name: "Search commands", exact: true }).fill("Open File Properties");
  await palette.getByRole("option", { name: /Open File Properties/ }).click();
  const panel = page.getByTestId("file-properties-panel");
  const status = panel.getByRole("combobox", { name: "status value", exact: true });
  await expect(status).toHaveText("ready");
  return { panel, status, source };
}

async function propose(status: Locator, value: string, commit = true) {
  await status.fill(value);
  await expect(status).toHaveText(value);
  if (commit) await status.press("Enter");
}

async function logout(page: Page) {
  await manageVaults(page);
  await page.getByRole("button", { name: "Sign out", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Sign in", exact: true })).toBeVisible();
}

test("hosted Properties preserves a focused stale draft and recovers through shared Problems review", async ({ page }, info) => {
  const f = await createHostedTasksFixture(page, artifact, { "Hosted.md": original });
  const file = join(f.service.vault, "Hosted.md");
  try {
    await f.open({ tasks: false });
    const { panel, status } = await properties(page);
    await propose(status, "local", false);
    const external = original.replace("status: ready", "status: external\nowner: outside");
    await writeFile(file, external);
    await expect(panel.getByRole("combobox", { name: "owner value", exact: true })).toHaveText("outside");
    await expect(status).toHaveText("local");
    await status.press("Enter");
    const problem = page.getByText("File changed on disk. Review your draft before saving.", { exact: true });
    await expect(problem).toBeVisible();
    expect(await readFile(file, "utf8")).toBe(external);
    await problem.click();
    const review = page.getByRole("region", { name: "File conflict: Hosted.md", exact: true });
    await expect(review).toBeVisible();
    await expect(review.getByRole("region", { name: "Your version", exact: true })).toContainText("status: local");
    await expect(review.getByRole("region", { name: "Disk version", exact: true })).toContainText("owner: outside");
    await review.getByRole("button", { name: "Accept All Incoming Changes from Right", exact: true }).click();
    await review.getByRole("button", { name: "Save merged version", exact: true }).click();
    await expect(review).toHaveCount(0);
    await expect(problem).toHaveCount(0);
    expect(await readFile(file, "utf8")).toBe(external);
    await expect(status).toHaveText("external");
    await propose(status, "after recovery");
    await expect.poll(() => readFile(file, "utf8")).toContain("status: after recovery");
    expect(await readFile(file, "utf8")).toContain("owner: outside");
    await logout(page);
  } finally {
    await f.stop(info);
  }
});

test("hosted Properties preserves a separately saved body and leaves normally", async ({ page }, info) => {
  const f = await createHostedTasksFixture(page, artifact, { "Hosted.md": original });
  const file = join(f.service.vault, "Hosted.md");
  try {
    await f.open({ tasks: false });
    const { status, source } = await properties(page);
    await propose(status, "local", false);
    await writeFile(file, `${original}\nA separately saved body change.\n`);
    await expect(source).toContainText("A separately saved body change.");
    await status.press("Enter");
    await expect.poll(() => readFile(file, "utf8")).toContain("status: local");
    expect(await readFile(file, "utf8")).toContain("A separately saved body change.");
    await expect(page.getByText("File changed on disk. Review your draft before saving.", { exact: true })).toHaveCount(0);
    await logout(page);
    expect(await readFile(file, "utf8")).toContain("A separately saved body change.");
  } finally {
    await f.stop(info);
  }
});
