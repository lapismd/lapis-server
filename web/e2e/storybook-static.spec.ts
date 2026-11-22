import { expect, test, type Page } from "@playwright/test";
import { createServer, type Server } from "node:http";
import { access, readFile } from "node:fs/promises";
import { extname, join, resolve, sep } from "node:path";

const root = resolve(import.meta.dirname, "../../storybook-static");
const contentTypes: Record<string, string> = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".json": "application/json",
  ".svg": "image/svg+xml",
  ".wasm": "application/wasm",
  ".woff2": "font/woff2",
};
let server: Server;
let origin: string;

test.beforeAll(async () => {
  await access(join(root, "index.html"));
  server = createServer(async (request, response) => {
    try {
      let pathname = new URL(request.url!, "http://localhost").pathname;
      if (pathname.endsWith("/")) pathname += "index.html";
      const path = resolve(root, "." + pathname);
      if (!path.startsWith(root + sep)) throw Error("Outside static catalog");
      const bytes = await readFile(path);
      response.writeHead(200, {
        "content-type":
          contentTypes[extname(path)] ?? "application/octet-stream",
      });
      response.end(bytes);
    } catch {
      response.writeHead(404);
      response.end();
    }
  });
  await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
  const address = server.address();
  if (!address || typeof address === "string") throw Error("No catalog port");
  origin = `http://127.0.0.1:${address.port}`;
});

test.afterAll(async () => {
  if (server)
    await new Promise<void>((done, reject) =>
      server.close((error) => (error ? reject(error) : done())),
    );
});

function runtimeErrors(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (
      ["error", "warning"].includes(message.type()) &&
      /svelte/i.test(message.text())
    )
      errors.push(message.text());
  });
  return errors;
}

test("static Sync consent keeps expired requests bounded and unavailable at narrow width", async ({
  page,
}) => {
  const errors = runtimeErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(
    `${origin}/iframe.html?id=shell-sync-account-consent--expired&viewMode=story`,
  );
  await expect(page.getByRole("status")).toContainText("expired");
  await expect(
    page.getByRole("button", { name: "Approve connection" }),
  ).toHaveCount(0);
  const card = await page.locator(".server-login-card").boundingBox();
  expect(card).not.toBeNull();
  expect(card!.x).toBeGreaterThanOrEqual(0);
  expect(card!.x + card!.width).toBeLessThanOrEqual(390);
  expect(errors).toEqual([]);
});

test("static login loads its branding and remains centered", async ({
  page,
}) => {
  const errors = runtimeErrors(page);
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto(
    `${origin}/iframe.html?id=shell-hosted-application--login&viewMode=story`,
  );
  await expect(
    page.getByRole("heading", { name: "Sign in", exact: true }),
  ).toBeVisible();
  await expect
    .poll(() =>
      page
        .locator("[data-server-login-logo]")
        .evaluate((image: HTMLImageElement) => image.naturalWidth),
    )
    .toBeGreaterThan(0);
  const card = await page.locator(".server-login-card").boundingBox();
  expect(card).not.toBeNull();
  expect(Math.abs(card!.x + card!.width / 2 - 640)).toBeLessThan(1);
  expect(Math.abs(card!.y + card!.height / 2 - 360)).toBeLessThan(1);
  expect(errors).toEqual([]);
});

for (const [story, width, height] of [
  ["workspace", 1280, 720],
  ["workspace-dark", 390, 844],
] as const) {
  test(`static ${story} fills its ${width}px viewport`, async ({ page }) => {
    const errors = runtimeErrors(page);
    await page.setViewportSize({ width, height });
    await page.goto(
      `${origin}/iframe.html?id=shell-hosted-application--${story}&viewMode=story`,
    );
    await expect(
      page.locator('.cm-content[contenteditable="true"]'),
    ).toBeVisible();
    const shell = page.locator('[data-ui-component="lapis-workspace-shell"]');
    await expect
      .poll(async () => (await shell.boundingBox())?.height)
      .toBe(height);
    expect((await shell.boundingBox())?.width).toBe(width);
    if (story.endsWith("dark"))
      await expect(page.locator("html")).toHaveClass(/dark/);
    expect(errors).toEqual([]);
  });
}

test("static Docs isolate complete shells in 700px embeds", async ({
  page,
}) => {
  const errors = runtimeErrors(page);
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto(`${origin}/?path=/docs/shell-hosted-application--docs`);
  const docs = page.frameLocator("#storybook-preview-iframe");
  const embeds = docs.locator(".server-docs-canvas iframe");
  await expect(embeds.first()).toBeVisible();
  const heights = await embeds.evaluateAll((frames) =>
    frames.map((frame) => frame.getBoundingClientRect().height),
  );
  expect(heights.length).toBeGreaterThan(0);
  for (const height of heights) expect(height).toBe(700);
  expect(errors).toEqual([]);
});

test("static MCP Apps show the packaged views and support sample interactions", async ({
  page,
}) => {
  const errors = runtimeErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(
    `${origin}/iframe.html?id=shell-mcp-apps--notes&viewMode=story`,
  );
  const note = page.frameLocator('iframe[title="Lapis Notes"]');
  await expect(
    note.getByRole("heading", { name: /Team offsite/ }),
  ).toBeVisible();
  await expect(note.getByRole("table")).toBeVisible();
  await expect(
    note.getByRole("navigation", { name: "breadcrumb" }),
  ).toContainText("Projects");
  await expect(
    note.getByRole("link", { name: "Team Offsite.md" }),
  ).toHaveAttribute("aria-current", "page");
  await expect(note.locator(".markdown-view__document")).toHaveCSS(
    "padding-bottom",
    "0px",
  );
  await expect(note.getByRole("button", { name: "Refresh" })).toHaveText("");
  await expect(note.locator("body")).toHaveCSS(
    "background-color",
    "rgb(255, 255, 255)",
  );
  await note.getByRole("button", { name: "Refresh" }).click();
  await expect(note.getByText(/Latest update/)).toBeVisible();

  await page.goto(
    `${origin}/iframe.html?id=shell-mcp-apps--tasks&viewMode=story`,
  );
  if (
    (await page.getByTestId("mcp-app").getAttribute("data-unavailable")) ===
    "true"
  ) {
    await expect(page.getByRole("status")).toHaveText(
      "The installed Tasks plugin does not contribute an MCP App.",
    );
    expect(errors).toEqual([]);
    return;
  }
  const tasks = page.frameLocator('iframe[title="Lapis Tasks"]');
  await expect(tasks.locator("body")).toHaveCSS(
    "background-color",
    "rgb(255, 255, 255)",
  );
  await expect(tasks.getByRole("button", { name: "Refresh tasks" })).toHaveText(
    "",
  );
  const row = tasks.locator('[data-planner-task-id="prototype"]');
  const surface = tasks.getByRole("region", { name: "Selected task" });
  await expect(
    tasks.getByRole("progressbar", { name: "Checklist progress: 1 of 3" }),
  ).toBeVisible();
  await tasks
    .getByRole("button", {
      name: "Show checklist for Practice the prototype walkthrough",
      exact: true,
    })
    .click();
  await expect(
    tasks.getByRole("checkbox", { name: "Review the demo outline" }),
  ).toBeDisabled();
  await row
    .getByText("Practice the prototype walkthrough", { exact: true })
    .dblclick();
  await expect(
    row.getByRole("region", { name: "Selected task" }),
  ).toBeVisible();
  await row.getByRole("button", { name: "Close inline details" }).click();
  await expect(surface).toHaveCount(0);
  await tasks
    .getByRole("button", {
      name: "Open task details for Practice the prototype walkthrough",
      exact: true,
    })
    .click();
  await expect(
    tasks.getByRole("button", { name: "Close Task details column" }),
  ).toBeVisible();
  await expect(surface.getByRole("textbox", { name: "Task title" })).toHaveText(
    "Practice the prototype walkthrough",
  );
  await expect(row).toHaveAttribute("data-planner-selected", "true");
  await expect(surface).toHaveAttribute("data-task-sidebar-detail", "");
  await expect(surface.getByRole("button", { name: "Comments" })).toBeVisible();
  await expect(
    surface.getByRole("button", { name: "Task actions" }),
  ).toBeDisabled();
  await surface.getByRole("button", { name: "Properties" }).click();
  await surface.getByRole("checkbox", { name: "In Inbox" }).click();
  await expect(
    surface.getByRole("checkbox", { name: "In Inbox" }),
  ).toBeChecked();
  await surface.getByRole("checkbox", { name: "Mark task done" }).click();
  const dialog = tasks.getByRole("dialog", { name: /Complete Practice/ });
  await dialog.getByLabel("Duration").fill("25m");
  await dialog.getByRole("button", { name: "Complete", exact: true }).click();
  await expect(
    tasks.getByText("Task completed.", { exact: true }),
  ).toBeVisible();
  await expect(
    surface.getByRole("checkbox", { name: "Mark task incomplete" }),
  ).toBeChecked();
  expect(
    await tasks
      .locator("html")
      .evaluate((element) => element.scrollWidth <= element.clientWidth),
  ).toBe(true);
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.reload();
  await tasks
    .getByRole("button", {
      name: "Open task details for Review the offsite agenda",
      exact: true,
    })
    .click();
  const planner = tasks.locator(
    '[data-ui-part="column"][data-column-id="planner"]',
  );
  const detail = tasks.locator(
    '[data-ui-part="column"][data-column-id="detail"]',
  );
  await expect(
    detail.getByRole("region", { name: "Selected task" }),
  ).toBeVisible();
  await expect
    .poll(async () => {
      const [left, right] = await Promise.all([
        planner.boundingBox(),
        detail.boundingBox(),
      ]);
      return Boolean(left && right && right.x > left.x);
    })
    .toBe(true);
  await expect.soft(detail).toHaveCSS("border-top-width", "0px");
  await expect
    .soft(detail.getByRole("region", { name: "Selected task" }))
    .toHaveCSS("border-top-width", "1px");
  const canvas = tasks.getByRole("region", { name: "Task planner" });
  expect.soft((await canvas.boundingBox())?.height).toBeGreaterThanOrEqual(500);
  const listInset = await planner
    .getByRole("list", { name: "Tasks" })
    .evaluate(
      (list) =>
        list.getBoundingClientRect().top -
        list.closest("[data-scroll-area-viewport]")!.getBoundingClientRect()
          .top,
    );
  expect.soft(listInset).toBeGreaterThanOrEqual(8);
  expect(errors).toEqual([]);
});
