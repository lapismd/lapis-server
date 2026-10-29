import { test, expect } from "@playwright/test";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { randomBytes } from "node:crypto";
import { resolve, join } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";

test("the retained hosted workspace observes native metadata and Search across detach and disabled replacement", async ({
  page,
  context,
}, testInfo) => {
  const repository = resolve("../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  const root = await mkdtemp(join(repository, ".local/rust-hosted-metadata-"));
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const password = randomBytes(32).toString("base64url");
  const runtime = new RustServerRuntime(root, repository, origin, password);
  const indexing: Array<{
    url: string;
    kind: string;
    selection?: { providers: Array<{ provider_id: string }> };
    rebuild?: boolean;
  }> = [];
  const clientWrites: string[] = [];
  const nativeFailures: number[] = [];
  page.on("response", (response) => {
    if (response.url().endsWith("/indexing") && !response.ok())
      nativeFailures.push(response.status());
  });
  page.on("request", (request) => {
    if (request.method() !== "POST") return;
    if (request.url().endsWith("/indexing"))
      indexing.push({ ...request.postDataJSON(), url: request.url() });
    if (request.url().endsWith("/database")) {
      const call = request.postDataJSON();
      if (
        [
          "upsertIndexedFile",
          "deleteIndexedFile",
          "renameIndexedFile",
          "saveMetadataSnapshot",
          "upsertSearchDocument",
          "deleteSearchDocument",
          "rebuildSearchIndex",
          "beginSearchIndexingBatch",
          "endSearchIndexingBatch",
        ].includes(call.method) ||
        (call.method === "setMeta" &&
          [
            "metadata-cache.reconcile-checkpoint.v1",
            "search.reconcile-checkpoint.v1",
          ].includes(call.args[0]?.value))
      )
        clientWrites.push(call.method);
    }
  });
  try {
    await runtime.start();
    await page.goto(origin);
    await page.getByLabel("Username").fill("owner");
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await expect(page.getByText("Server DB", { exact: true })).toBeVisible();
    await expect
      .poll(() => indexing.some(({ kind }) => kind === "select_metadata"))
      .toBe(true);
    await expect
      .poll(() => indexing.some(({ kind }) => kind === "refresh"))
      .toBe(true);
    await expect
      .poll(() => indexing.some(({ kind }) => kind === "select_search"))
      .toBe(true);
    const selectedSearch = indexing.find(
      ({ kind }) => kind === "select_search",
    )!;
    expect(
      selectedSearch.selection?.providers
        .map(({ provider_id }) => provider_id)
        .sort(),
    ).toEqual(["search:canvas", "search:markdown"]);
    const selected = indexing.find(({ kind }) => kind === "select_metadata")!;
    const clientPath = selected.url.slice(0, -"/indexing".length);
    expect(
      indexing.filter(({ kind }) => kind === "select_metadata"),
    ).toHaveLength(1);
    const current = await context.request.get(`${origin}/api/v3/auth/current`, {
      headers: { "x-lapis-protocol": "3" },
    });
    const { csrf_token } = await current.json();
    const headers = {
      "x-lapis-protocol": "3",
      "x-lapis-csrf": csrf_token,
      origin,
    };
    const record = async (client: string, path: string) => {
      const response = await context.request.post(`${client}/database`, {
        headers,
        data: {
          method: "getIndexedFile",
          args: [{ kind: "value", value: path }],
        },
      });
      expect(response.ok()).toBe(true);
      const value = await response.json();
      return value.kind === "value" ? value.value : null;
    };
    await expect
      .poll(
        async () =>
          (await record(clientPath, "Note.md"))?.metadata.metadata.headings[0]
            ?.heading,
      )
      .toBe("Native hosted note");
    const fenced = await context.request.post(`${clientPath}/database`, {
      headers,
      data: {
        method: "deleteIndexedFile",
        args: [{ kind: "value", value: "Note.md" }],
      },
    });
    expect(fenced.status()).toBe(403);
    await page.locator('[data-path="Note.md"]').click();
    const editor = page.locator(".cm-content[contenteditable=true]").last();
    const text = "# Saved with native metadata\n\n#hosted\n";
    await editor.fill(text);
    await expect
      .poll(() => readFile(join(runtime.vault, "Note.md"), "utf8"))
      .toBe(text);
    await expect
      .poll(
        async () =>
          (await record(clientPath, "Note.md"))?.metadata.metadata.headings[0]
            ?.heading,
      )
      .toBe("Saved with native metadata");
    await page.getByRole("tab", { name: "Search", exact: true }).click();
    await page
      .getByRole("searchbox", { name: "Search vault" })
      .fill("observer delivery after native publication");
    await writeFile(
      join(runtime.vault, "Observer.md"),
      "# Observer\n\nobserver delivery after native publication\n",
    );
    await expect(
      page.getByRole("treeitem", { name: /Observer\.md,/ }),
    ).toBeVisible();
    expect(clientWrites).toEqual([]);
    await writeFile(
      join(runtime.vault, "Fruits.canvas"),
      JSON.stringify({ nodes: [{ text: "native canvas pear" }] }),
    );
    await page
      .getByRole("searchbox", { name: "Search vault" })
      .fill("native canvas pear");
    await expect(
      page.getByRole("treeitem", { name: /Fruits\.canvas,/ }),
    ).toBeVisible();
    const searchFenced = await context.request.post(`${clientPath}/database`, {
      headers,
      data: {
        method: "deleteSearchDocument",
        args: [{ kind: "value", value: "Fruits.canvas" }],
      },
    });
    expect(searchFenced.status()).toBe(403);
    const refreshes = indexing.filter(({ kind }) => kind === "refresh").length;
    await page
      .getByRole("button", { name: "Refresh search index", exact: true })
      .click();
    await expect
      .poll(() => indexing.filter(({ kind }) => kind === "refresh").length)
      .toBeGreaterThan(refreshes);
    await page
      .getByRole("button", { name: "Open command palette", exact: true })
      .click();
    const palette = page.getByRole("dialog", { name: "Command Palette" });
    await palette
      .getByRole("combobox", { name: "Search commands" })
      .fill("Rebuild semantic search embeddings");
    await palette
      .getByRole("option", { name: /Rebuild semantic search embeddings/ })
      .click();
    await expect
      .poll(() =>
        indexing.some(({ kind, rebuild }) => kind === "refresh" && rebuild),
      )
      .toBe(true);
    await expect(palette).toHaveCount(0);
    await expect(page.getByText(/Semantic search refreshed \(/)).toBeVisible();
    expect(clientWrites).toEqual([]);

    await page
      .getByRole("button", { name: "Expand filter options", exact: true })
      .click();
    const showContext = page.getByRole("switch", {
      name: "Show more context",
      exact: true,
    });
    await expect(showContext).toHaveAttribute("aria-checked", "false");
    const appConfig = join(runtime.vault, ".lapis/app.json");
    await expect
      .poll(async () => {
        const value = JSON.parse(await readFile(appConfig, "utf8"));
        return value.pluginData?.search?.view?.recentSearches;
      })
      .toContain("native canvas pear");
    const recovered = JSON.parse(await readFile(appConfig, "utf8"));
    recovered.pluginData.search.view.showMoreContext = true;
    recovered.pluginData.search.view.matchCase = false;
    recovered.hostedSearchRecovery = "retained";
    await writeFile(appConfig, JSON.stringify(recovered, null, 2));
    await expect(showContext).toHaveAttribute("aria-checked", "true");
    await expect(
      page.getByRole("searchbox", { name: "Search vault" }),
    ).toHaveText("native canvas pear");
    await page
      .getByRole("switch", { name: "Collapse results", exact: true })
      .click();
    await expect
      .poll(async () => {
        const value = JSON.parse(await readFile(appConfig, "utf8"));
        return {
          matchCase: value.pluginData?.search?.view?.matchCase,
          showMoreContext: value.pluginData?.search?.view?.showMoreContext,
          collapseResults: value.pluginData?.search?.view?.collapseResults,
          retained: value.hostedSearchRecovery,
        };
      })
      .toEqual({
        matchCase: false,
        showMoreContext: true,
        collapseResults: false,
        retained: "retained",
      });

    expect(nativeFailures).toEqual([]);
    const detached = await context.request.delete(clientPath, { headers });
    expect(detached.status()).toBe(204);
    await page.close();
    await writeFile(
      join(runtime.vault, "After-detach.md"),
      "# Native after detach\n",
    );
    const attached = await context.request.post(
      `${origin}/api/v3/vaults/${runtime.vaultId}/clients`,
      {
        headers,
        data: { protocol_version: 3 },
      },
    );
    expect(attached.status()).toBe(200);
    const { client_id } = await attached.json();
    const observer = `${origin}/api/v3/clients/${client_id}`;
    try {
      await expect
        .poll(
          async () =>
            (await record(observer, "After-detach.md"))?.metadata.metadata
              .headings[0]?.heading,
        )
        .toBe("Native after detach");
      const searchDocument = async (client: string, path: string) => {
        const response = await context.request.post(`${client}/database`, {
          headers,
          data: {
            method: "getSearchDocument",
            args: [{ kind: "value", value: path }],
          },
        });
        expect(response.ok()).toBe(true);
        const value = await response.json();
        return value.kind === "value" ? value.value : null;
      };
      await expect
        .poll(
          async () =>
            (await searchDocument(observer, "After-detach.md"))?.content,
        )
        .toContain("Native after detach");
      expect(await readFile(join(runtime.vault, "Note.md"), "utf8")).toBe(text);
      const configPath = join(runtime.vault, ".lapis/core-plugins.json");
      const core = JSON.parse(
        await readFile(configPath, "utf8").catch(() => "{}"),
      );
      await writeFile(
        configPath,
        JSON.stringify({
          ...core,
          disabled: [...new Set([...(core.disabled ?? []), "search"])],
          enabled: (core.enabled ?? []).filter((id: string) => id !== "search"),
        }),
      );
      const replacement = await context.newPage();
      const replacementCalls: string[] = [];
      replacement.on("request", (request) => {
        if (request.method() === "POST" && request.url().endsWith("/indexing"))
          replacementCalls.push(request.postDataJSON().kind);
      });
      try {
        await replacement.goto(origin);
        await replacement
          .getByRole("button", { name: "Open Rust vault", exact: true })
          .click();
        await expect(
          replacement.getByText("Server DB", { exact: true }),
        ).toBeVisible();
        await expect
          .poll(() => replacementCalls.includes("disable_search"))
          .toBe(true);
        expect(replacementCalls).not.toContain("select_search");
        await expect(
          replacement.getByRole("searchbox", { name: "Search vault" }),
        ).toHaveCount(0);
        await writeFile(
          join(runtime.vault, "Disabled.md"),
          "# Metadata after Search disable\n",
        );
        await expect
          .poll(
            async () =>
              (await record(observer, "Disabled.md"))?.metadata.metadata
                .headings[0]?.heading,
          )
          .toBe("Metadata after Search disable");
        expect(await searchDocument(observer, "Disabled.md")).toBeNull();
        const released = await context.request.post(`${observer}/database`, {
          headers,
          data: {
            method: "deleteSearchDocument",
            args: [{ kind: "value", value: "__disabled-probe.md" }],
          },
        });
        expect(released.status()).toBe(200);
        expect(await readFile(join(runtime.vault, "Note.md"), "utf8")).toBe(
          text,
        );
      } finally {
        await replacement.close();
      }
    } finally {
      expect(
        (await context.request.delete(observer, { headers })).status(),
      ).toBe(204);
    }
  } catch (error) {
    await testInfo.attach("Rust metadata host log", {
      body: runtime.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    await runtime.stop();
  }
});
