import { expect, test } from "@playwright/test";
import { randomBytes } from "node:crypto";
import { mkdir, mkdtemp, readFile, readdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustHttpClient } from "../src/hosted/rust/client";
import { RustDatabaseProvider } from "../src/hosted/rust/database-provider";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import {
  assertAssets,
  assertModel,
  qualifiedReference,
} from "./hosted-search-model-reference";

test("actual hosted ONNX provider retains qualified vectors and ranks across detach and cold offline restart", async ({
  page,
  context,
}, info) => {
  test.skip(
    process.env.LAPIS_TEST_HOSTED_ONNX !== "1",
    "Separate actual-model gate: set LAPIS_TEST_HOSTED_ONNX=1 on the qualified Linux worker",
  );
  test.setTimeout(240_000);
  const fixture = await qualifiedReference();
  const repository = resolve("../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  const root = await mkdtemp(join(repository, ".local/rust-hosted-onnx-"));
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const password = randomBytes(32).toString("base64url");
  const runtime = new RustServerRuntime(root, repository, origin, password);
  let client: Awaited<ReturnType<RustHttpClient["attach"]>> | undefined;
  let database: Awaited<ReturnType<RustDatabaseProvider["open"]>> | undefined;
  try {
    await runtime.start();
    await page.goto(origin);
    await page.getByLabel("Username").fill("owner");
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "Open Rust vault", exact: true }),
    ).toBeVisible();
    const open = async () => {
      const cookies = (await context.cookies(origin))
        .map(({ name, value }) => `${name}=${value}`)
        .join("; ");
      const host = new RustHttpClient(origin, (input, init) => {
        const headers = new Headers(init?.headers);
        headers.set("cookie", cookies);
        headers.set("origin", origin);
        return fetch(input, { ...init, headers });
      });
      await host.current();
      client = await host.attach(runtime.vaultId);
      database = await new RustDatabaseProvider(client).open({
        vaultId: runtime.vaultId,
        runtime: "rust-server",
        role: "direct",
      });
      return database;
    };
    const document = (index: number, text: string) => ({
      path: `${index}.md`,
      name: `Record ${index}`,
      extension: "md",
      checksum: "retained-model-reference",
      content: text,
      tags: [],
      tagParts: [],
      tagHierarchy: [],
      chunks: [
        { id: `${index}.md#0`, text, startOffset: 0, endOffset: text.length },
      ],
    });
    let db = await open();
    expect(await db.getSearchEmbeddingProvider()).toBeNull();
    expect(await readdir(join(root, "profile/models"))).toEqual([]);
    const configuration = {
      kind: "transformers-js" as const,
      modelId: fixture.modelId,
      revision: fixture.revision,
      dtype: fixture.dtype,
      dimensions: 384,
      normalize: true,
      allowRemoteModels: true,
    };
    await db.configureSearchEmbeddingProvider(configuration);
    for (const [index, row] of fixture.queries.entries())
      await db.upsertSearchDocument(document(index, row.text));
    await assertModel(db, fixture);
    const installed = await assertAssets(root, fixture);
    await db.close();
    await client!.close();
    db = await open();
    await assertModel(db, fixture);
    const offline = { ...configuration, allowRemoteModels: false };
    await db.configureSearchEmbeddingProvider(offline);
    for (const [index, row] of fixture.queries.entries())
      await db.upsertSearchDocument(document(index, row.text));
    await assertModel(db, fixture);
    await db.close();
    await client!.close();
    client = undefined;
    await runtime.restart();
    db = await open();
    expect(await db.getSearchEmbeddingProvider()).toEqual(offline);
    await assertModel(db, fixture);
    expect(await assertAssets(root, fixture)).toEqual(installed);
    expect(await readdir(runtime.vault)).toEqual(["Note.md"]);
    expect(await readFile(join(runtime.vault, "Note.md"), "utf8")).toBe(
      "# Native hosted note\n\nRead and edit this note through Rust.\n",
    );
  } finally {
    await database?.close();
    await client?.close();
    await runtime.stop();
    await info.attach("hosted-onnx-service.log", {
      body: runtime.logs,
      contentType: "text/plain",
    });
  }
});
