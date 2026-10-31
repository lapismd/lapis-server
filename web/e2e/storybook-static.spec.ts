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
