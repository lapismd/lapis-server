import { test, expect, type Page } from "@playwright/test";
import { mkdtemp, mkdir, readFile, writeFile, readdir } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { randomBytes, createHash } from "node:crypto";
import { resolve, join } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";

async function metadata(root: string): Promise<Record<string, string>> {
  const result: Record<string, string> = {};
  async function visit(path: string) {
    for (const entry of await readdir(join(root, path), {
      withFileTypes: true,
    })) {
      const child = join(path, entry.name);
      if (entry.isDirectory()) {
        result[child] = "directory";
        await visit(child);
      } else
        result[child] = createHash("sha256")
          .update(await readFile(join(root, child)))
          .digest("hex");
    }
  }
  await visit(".jj");
  await visit(".git");
  return result;
}
function jj(root: string, args: string[]) {
  return execFileSync(
    "jj",
    [
      "--config",
      'user.name="Source fixture"',
      "--config",
      'user.email="source@example.invalid"',
      ...args,
    ],
    { cwd: root, encoding: "utf8" },
  );
}
async function command(page: Page, name: string) {
  await page
    .getByRole("button", { name: "Open command palette", exact: true })
    .click();
  const palette = page.getByRole("dialog", { name: "Command Palette" });
  await palette.getByRole("combobox", { name: "Search commands" }).fill(name);
  await palette.getByRole("option", { name: new RegExp(name) }).click();
  await expect(palette).toHaveCount(0);
}

test("retained independent original tabs preserve pins, dirty bytes and metadata across refresh and cold restart", async ({
  page,
}, testInfo) => {
  const repository = resolve("../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  const root = await mkdtemp(join(repository, ".local/rust-hosted-original-"));
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const password = randomBytes(32).toString("base64url");
  const runtime = new RustServerRuntime(root, repository, origin, password);
  const saved = "# Original note\n\nOriginal source has saffron history.\n";
  const dirty = "# Dirty original\n\nThese bytes must remain unsnapshotted.\n";
  await mkdir(runtime.vault, { recursive: true });
  await writeFile(join(runtime.vault, "Note.md"), saved);
  jj(runtime.vault, ["git", "init", "--colocate"]);
  jj(runtime.vault, ["describe", "-m", "Original checkpoint"]);
  const pin = jj(runtime.vault, [
    "--ignore-working-copy",
    "op",
    "log",
    "--limit",
    "1",
    "--no-graph",
    "-T",
    "id",
  ]).trim();
  await mkdir(join(runtime.vault, ".lapis"), { recursive: true });
  await writeFile(
    join(runtime.vault, ".lapis/app.json"),
    JSON.stringify({
      pluginData: { "repository-history": { automaticBackup: false } },
    }),
  );
  await writeFile(join(runtime.vault, "Note.md"), dirty);
  let expectedMetadata = await metadata(runtime.vault);
  const sources: string[] = [],
    scopes: string[] = [],
    failures: string[] = [];
  page.on("request", (request) => {
    if (request.method() !== "POST") return;
    if (request.url().endsWith("/repository-sources"))
      sources.push(request.postDataJSON().kind);
    if (/\/history\//u.test(request.url()))
      scopes.push(request.postDataJSON().data.vault);
  });
  page.on("response", (response) => {
    if (
      /\/(?:history\/|repository-sources)/u.test(response.url()) &&
      !response.ok()
    )
      failures.push(`${response.status()} ${response.url()}`);
  });
  const open = async () => {
    await page.goto(origin);
    await expect(
      page.getByRole("heading", { name: /^(Sign in|Your vaults)$/u }),
    ).toBeVisible();
    if (await page.getByLabel("Username").count()) {
      await page.getByLabel("Username").fill("owner");
      await page.getByLabel("Password").fill(password);
      await page.getByRole("button", { name: "Sign in", exact: true }).click();
    }
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await expect(page.getByText("Server DB", { exact: true })).toBeVisible();
  };
  const history = page.getByRole("region", {
    name: "Repository history",
    exact: true,
  });
  const selectSource = async (
    current: "Backup" | "Original",
    label: string,
    region = history,
  ) => {
    const trigger = region.getByRole("button", {
      name: `History source: ${current}`,
      exact: true,
    });
    await trigger.click();
    await expect(trigger).toHaveAttribute("aria-expanded", "true");
    const controlled = await trigger.getAttribute("aria-controls");
    expect(controlled).toBeTruthy();
    const popup = page.locator(`[id=${JSON.stringify(controlled)}]`);
    await popup.getByRole("option", { name: new RegExp(label) }).click();
    await expect(popup).toBeHidden();
  };
  try {
    await runtime.start();
    await open();
    await command(page, "Open Repository History");
    await expect(history).toContainText("No repository operations available.");
    await selectSource("Backup", "Original repository");
    await expect(
      history.getByRole("button", {
        name: "History source: Original",
        exact: true,
      }),
    ).toBeVisible();
    const operation = history.getByRole("combobox", {
      name: "Operation",
      exact: true,
    });
    await expect(operation).toHaveAttribute("title", `Pinned operation ${pin}`);
    const graph = history.getByRole("list", {
      name: "Revision graph",
      exact: true,
    });
    await expect(
      graph.getByText("Original checkpoint", { exact: true }),
    ).toBeVisible();
    await operation.click();
    await page
      .getByRole("row", { name: `Select operation ${pin}`, exact: true })
      .click();
    await graph.locator("button.repository-graph-select").first().click();
    await history
      .getByRole("region", { name: "Changed files", exact: true })
      .getByText("Note.md", { exact: true })
      .click();
    await expect(
      history.getByRole("region", { name: "Changes to Note.md", exact: true }),
    ).toContainText("Original source has saffron history.");
    await expect(
      history.getByRole("button", {
        name: "Restore this file version",
        exact: true,
      }),
    ).toHaveCount(0);
    expect(await metadata(runtime.vault)).toEqual(expectedMetadata);
    expect(await readFile(join(runtime.vault, "Note.md"), "utf8")).toBe(dirty);
    await selectSource("Original", "Automatic vault history");
    await expect(history).toContainText("No repository operations available.");
    await selectSource("Backup", "Original repository");
    await expect(operation).toHaveAttribute("title", `Pinned operation ${pin}`);
    // This is the sole source writer: a normal external JJ operation.
    jj(runtime.vault, ["describe", "-m", "Externally advanced source"]);
    const externalDescription = jj(runtime.vault, [
      "--ignore-working-copy",
      "op",
      "log",
      "--limit",
      "1",
      "--no-graph",
      "-T",
      "description",
    ]).trim();
    const externalPin = jj(runtime.vault, [
      "--ignore-working-copy",
      "op",
      "log",
      "--limit",
      "1",
      "--no-graph",
      "-T",
      "id",
    ]).trim();
    expectedMetadata = await metadata(runtime.vault);
    const externalDirty = await readFile(
      join(runtime.vault, "Note.md"),
      "utf8",
    );
    await history.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(operation).toHaveAttribute("title", `Pinned operation ${pin}`);
    await expect(
      graph.getByText("Original checkpoint", { exact: true }),
    ).toBeVisible();
    await command(page, "Open Repository Op Log");
    const log = page.getByRole("region", {
      name: "Repository operation log",
      exact: true,
    });
    await selectSource("Backup", "Original repository", log);
    await expect(
      log.getByRole("combobox", { name: "Pinned operation", exact: true }),
    ).toHaveAttribute("title", `Pinned operation ${externalPin}`);
    await expect(
      log
        .getByRole("list", { name: "Operation graph", exact: true })
        .getByRole("button", { name: new RegExp(`^${externalDescription};`) }),
    ).toBeVisible();
    await expect
      .poll(async () =>
        JSON.stringify(
          JSON.parse(
            await readFile(
              join(runtime.vault, ".lapis/workspace.json"),
              "utf8",
            ),
          ),
        ),
      )
      .toContain('"source":"original"');
    await page.goto("about:blank");
    await runtime.restart();
    await open();
    await command(page, "Open Repository History");
    await selectSource("Backup", "Original repository");
    await expect(operation).toHaveAttribute("title", `Pinned operation ${pin}`);
    await expect(
      graph.getByText("Original checkpoint", { exact: true }),
    ).toBeVisible();
    expect(await metadata(runtime.vault)).toEqual(expectedMetadata);
    expect(await readFile(join(runtime.vault, "Note.md"), "utf8")).toBe(
      externalDirty,
    );
    expect(sources).toEqual(
      expect.arrayContaining(["discover", "open_original"]),
    );
    expect(scopes.some((scope) => scope === runtime.vaultId)).toBe(true);
    expect(scopes.some((scope) => scope.startsWith("original:"))).toBe(true);
    expect(failures).toEqual([]);
    await page.goto("about:blank");
  } catch (error) {
    await testInfo.attach("Rust original source host log", {
      body: runtime.logs,
      contentType: "text/plain",
    });
    await testInfo.attach("Source admissions", {
      body: JSON.stringify({ sources, scopes, failures }),
      contentType: "application/json",
    });
    throw error;
  } finally {
    await runtime.stop();
  }
});
