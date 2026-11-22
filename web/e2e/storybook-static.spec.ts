import { expect, test, type FrameLocator, type Page } from "@playwright/test";
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

test("static MCP Apps Notes shows and refreshes the packaged preview", async ({
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

  expect(errors).toEqual([]);
});

async function openStaticTasks(page: Page) {
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
    return;
  }
  return page.frameLocator('iframe[title="Lapis Tasks"]');
}

async function openPrototypeTaskDetails(tasks: FrameLocator) {
  await tasks
    .getByRole("button", {
      name: "Open task details for Practice the prototype walkthrough",
      exact: true,
    })
    .click();
  return tasks.getByRole("region", { name: "Selected task" });
}

test("static MCP Apps Tasks opens and dismisses inline details", async ({
  page,
}) => {
  const errors = runtimeErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  const tasks = await openStaticTasks(page);
  if (!tasks) {
    expect(errors).toEqual([]);
    return;
  }
  await expect(tasks.locator("body")).toHaveCSS(
    "background-color",
    "rgb(255, 255, 255)",
  );
  await expect(tasks.getByRole("button", { name: "Refresh tasks" })).toHaveText(
    "",
  );
  const row = tasks.locator('[data-planner-task-id="prototype"]');
  await expect(
    row.getByText("Practice the prototype walkthrough"),
  ).toBeVisible();
  const rowOpacity = await row.evaluate((element) =>
    [".task-line__title", ".task-line__due", ".task-when-picker__trigger"].map(
      (selector) => {
        let opacity = 1;
        const text = element.querySelector(selector);
        if (!text) throw new Error(`Missing task text: ${selector}`);
        for (let node: Element | null = text; node; node = node.parentElement) {
          opacity *= Number(getComputedStyle(node).opacity);
          if (node === element) break;
        }
        return opacity;
      },
    ),
  );
  expect.soft(rowOpacity).toEqual([1, 1, 1]);
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
  const inline = row.locator("[data-task-preview-surface]");
  await expect(inline).toBeVisible();
  await expect
    .soft(inline.locator("[data-task-sidebar-detail]"))
    .toHaveCount(0, { timeout: 1000 });
  await expect
    .soft(inline.getByText("Rehearse the demo in the studio."))
    .toBeVisible({ timeout: 1000 });
  await expect
    .soft(
      inline.getByRole("button", { name: "Set planner schedule", exact: true }),
    )
    .toBeVisible({ timeout: 1000 });
  await inline
    .getByRole("button", { name: "Set planner schedule", exact: true })
    .click();
  await tasks.getByRole("button", { name: "Tomorrow", exact: true }).click();
  await expect(tasks.getByText("Task updated.", { exact: true })).toBeVisible();
  await expect(
    row.getByRole("checkbox", {
      name: "Mark Practice the prototype walkthrough complete",
    }),
  ).toBeEnabled();
  await expect
    .soft(row.getByRole("button", { name: "Close inline details" }))
    .toHaveCount(0, { timeout: 1000 });
  await tasks.getByText("Lapis Tasks", { exact: true }).click();
  await expect(inline).toHaveCount(0);
  await expect(surface).toHaveCount(0);
  await row
    .getByText("Practice the prototype walkthrough", { exact: true })
    .dblclick();
  await expect(
    inline.getByText("Rehearse the demo in the studio."),
  ).toBeVisible();
  await tasks.getByRole("button", { name: "Refresh tasks" }).focus();
  await expect(inline).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("static MCP Apps Tasks renders the sidebar and updates Inbox", async ({
  page,
}) => {
  const errors = runtimeErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  const tasks = await openStaticTasks(page);
  if (!tasks) {
    expect(errors).toEqual([]);
    return;
  }
  const row = tasks.locator('[data-planner-task-id="prototype"]');
  const surface = await openPrototypeTaskDetails(tasks);
  await expect(
    tasks.getByRole("button", { name: "Close Task details column" }),
  ).toBeVisible();
  await expect(surface.getByRole("textbox", { name: "Task title" })).toHaveText(
    "Practice the prototype walkthrough",
  );
  await expect(row).toHaveAttribute("data-planner-selected", "true");
  await expect(surface).toHaveAttribute("data-task-sidebar-detail", "");
  const sidebarNotes = surface.getByText("Rehearse the demo in the studio.");
  await expect(sidebarNotes).toBeVisible();
  const notesInset = await sidebarNotes.evaluate((notes) => {
    const viewport = notes.closest("[data-scroll-area-viewport]")!;
    const noteBounds = notes.getBoundingClientRect();
    const viewportBounds = viewport.getBoundingClientRect();
    const rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
    return [
      (noteBounds.left - viewportBounds.left) / rem,
      (viewportBounds.right - noteBounds.right) / rem,
    ];
  });
  expect.soft(notesInset).toEqual([1, 1]);
  await expect(surface.getByRole("button", { name: "Comments" })).toBeVisible();
  await expect(
    surface.getByRole("button", { name: "Task actions" }),
  ).toBeDisabled();
  await surface.getByRole("button", { name: "Expand properties" }).click();
  const properties = surface.getByTestId("mira-frontmatter-editor");
  await expect.soft(properties).toBeVisible({ timeout: 1000 });
  await expect
    .soft(properties.locator('[data-property="inbox"]'))
    .toHaveCSS("border-bottom-width", "1px", { timeout: 1000 });
  await expect(
    properties.getByRole("button", { name: "Add property" }),
  ).toBeDisabled();
  await expect(
    properties.getByRole("button", { name: "Property options for inbox" }),
  ).toBeDisabled();
  await surface.getByRole("checkbox", { name: "inbox value" }).click();
  await expect(
    surface.getByRole("checkbox", { name: "inbox value" }),
  ).toBeChecked();
  expect(errors).toEqual([]);
});

test("static MCP Apps Tasks completes a duration-tracked task", async ({
  page,
}) => {
  const errors = runtimeErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  const tasks = await openStaticTasks(page);
  if (!tasks) {
    expect(errors).toEqual([]);
    return;
  }
  const surface = await openPrototypeTaskDetails(tasks);
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
  expect(errors).toEqual([]);
});

test("static MCP Apps Tasks snaps between full-width compact panes", async ({
  page,
}) => {
  const errors = runtimeErrors(page);
  await page.setViewportSize({ width: 1280, height: 900 });
  const tasks = await openStaticTasks(page);
  if (!tasks) {
    expect(errors).toEqual([]);
    return;
  }
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
  await expect(canvas).toHaveAttribute("data-display-mode", "wide");
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
  await page.setViewportSize({ width: 820, height: 900 });
  await expect(canvas).toHaveAttribute("data-display-mode", "compact");
  const detailOpener = planner.getByRole("button", {
    name: "Open task details for Review the offsite agenda",
    exact: true,
  });
  await detail
    .getByRole("button", { name: "Close Task details column" })
    .click();
  await detailOpener.click();
  const columnFit = async (column: typeof detail) =>
    column.evaluate((panel) => {
      const root = panel.closest('[data-ui-part="root"]')!;
      const style = getComputedStyle(root);
      const start =
        root.getBoundingClientRect().left + parseFloat(style.paddingLeft);
      const end =
        start +
        root.clientWidth -
        parseFloat(style.paddingLeft) -
        parseFloat(style.paddingRight);
      const bounds = panel.getBoundingClientRect();
      return Math.max(
        Math.abs(bounds.left - start),
        Math.abs(bounds.right - end),
      );
    });
  await expect.poll(() => columnFit(detail)).toBeLessThanOrEqual(1);

  await detail.hover();
  await page.mouse.wheel(-1200, 0);
  await expect.poll(() => columnFit(planner)).toBeLessThanOrEqual(1);
  await expect(detail).toBeVisible();
  expect(errors).toEqual([]);
});
