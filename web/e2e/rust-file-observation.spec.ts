import { expect, test } from "@playwright/test";
import { randomBytes } from "node:crypto";
import { mkdir, mkdtemp, rename, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import {
  buildTasksArtifact,
  installTasksArtifact,
} from "./tasks-installed-plugin";
import {
  signInHostedOwner,
  signOutHostedWorkspace,
} from "./rust-hosted-session";

let artifact: string;
test.beforeAll(async () => {
  artifact = await buildTasksArtifact(resolve(import.meta.dirname, "../.."));
});
test("enabled plugin startup uses native file events and remains quiet while idle", async ({
  page,
}, info) => {
  const repository = resolve(import.meta.dirname, "../.."),
    root = await mkdtemp(join(repository, ".local/native-file-browser-"));
  const password = randomBytes(32).toString("base64url"),
    origin = `http://localhost:${await unusedLoopbackPort()}`;
  const runtime = new RustServerRuntime(root, repository, origin, password);
  runtime.historyIndexing = true;
  await mkdir(runtime.vault, { recursive: true });
  await installTasksArtifact(runtime.vault, artifact);
  await mkdir(join(runtime.vault, ".lapis/plugins/dormant/assets"), {
    recursive: true,
  });
  await mkdir(join(runtime.vault, ".lapis/plugin-registry-cache/fixture"), {
    recursive: true,
  });
  for (let i = 0; i < 600; i++)
    await writeFile(
      join(runtime.vault, `.lapis/plugins/dormant/assets/chunk-${i}.js`),
      "export {};\n",
    );
  for (let i = 0; i < 200; i++)
    await writeFile(
      join(
        runtime.vault,
        `.lapis/plugin-registry-cache/fixture/record-${i}.json`,
      ),
      "{}\n",
    );
  const calls: {
      at: number;
      kind: string;
      operation?: string;
      path?: string;
    }[] = [],
    db: string[] = [],
    errors: string[] = [];
  let streams = 0;
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("request", (request) => {
    if (request.url().includes("/file-events?")) streams++;
    if (request.method() !== "POST") return;
    if (request.url().endsWith("/database"))
      db.push(request.postDataJSON().method);
    if (request.url().endsWith("/commands")) {
      const c = request.postDataJSON().command;
      calls.push({
        at: Date.now(),
        kind: c.kind,
        operation: c.operation?.operation,
        path: c.operation?.path,
      });
    }
  });
  await runtime.start();
  try {
    await page.goto(origin);
    await signInHostedOwner(page, password);
    await page
      .getByRole("button", {
        name: "Open Rust vault owner · server-hosted",
        exact: true,
      })
      .click();
    await expect(
      page.locator('[data-notes-session-ready="true"]'),
    ).toBeVisible();
    await expect.poll(() => streams).toBeGreaterThan(0);
    // Preserve a full 20-second startup observation, then separately measure
    // steady idle cost. Neither sample scales with cached plugin artifacts.
    await page.waitForTimeout(20_000);
    const idleStart = Date.now(),
      beforeDb = db.length,
      beforeStreams = streams;
    await page.waitForTimeout(20_000);
    const idle = calls.filter(
      (c) =>
        c.at >= idleStart &&
        c.kind === "files" &&
        ["list", "stat"].includes(c.operation || ""),
    );
    expect(idle, "idle must not recurse through files over HTTP").toEqual([]);
    const idleDatabase = db.length - beforeDb;
    expect(idleDatabase, "idle must not poll the database").toBe(0);
    expect(
      streams - beforeStreams,
      "bounded lease reconnects must not churn",
    ).toBeLessThanOrEqual(2);
    expect(
      calls.filter((c) => c.path?.includes("plugin-registry-cache/fixture")),
    ).toEqual([]);
    await writeFile(join(runtime.vault, "External.md"), "# External\n");
    await expect(page.locator('button[data-path="External.md"]')).toBeVisible();
    await page.locator('button[data-path="External.md"]').click();
    const editor = page.locator(".cm-content[contenteditable=true]").last();
    await expect(editor).toContainText("External");
    await writeFile(
      join(runtime.vault, "External.md"),
      "# Updated externally\n",
    );
    await expect(editor).toContainText("Updated externally");
    await rename(
      join(runtime.vault, "External.md"),
      join(runtime.vault, "Renamed.md"),
    );
    await expect(page.locator('button[data-path="Renamed.md"]')).toBeVisible();
    await expect(page.locator('button[data-path="External.md"]')).toHaveCount(
      0,
    );
    await rm(join(runtime.vault, "Renamed.md"));
    await expect(page.locator('button[data-path="Renamed.md"]')).toHaveCount(0);
    expect(errors).toEqual([]);
    const receipt = info.outputPath("request-counts.json");
    await writeFile(
      receipt,
      JSON.stringify({
        commands: calls.length,
        database: db.length,
        streams,
        idleFilesystem: idle.length,
        idleDatabase,
        sampleMs: 20_000,
        dormantAssets: 600,
        registryCacheFiles: 200,
      }),
      "utf8",
    );
    await info.attach("native-watch-request-counts", {
      path: receipt,
      contentType: "application/json",
    });
  } catch (error) {
    await info.attach("host-log", {
      body: runtime.logs,
      contentType: "text/plain",
    });
    await info.attach("requests-at-failure", {
      body: JSON.stringify({ calls, db, streams, errors }),
      contentType: "application/json",
    });
    throw error;
  } finally {
    try {
      if (
        await page
          .getByRole("button", { name: /Current workspace:/ })
          .isVisible()
      )
        await signOutHostedWorkspace(page);
      await page.goto("about:blank");
    } finally {
      await runtime.stop();
    }
  }
});
