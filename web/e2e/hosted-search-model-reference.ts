import { expect } from "@playwright/test";
import type { AppDatabase } from "@lapis-notes/api/app-database";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { cpus } from "node:os";

type Reference = {
  modelId: string;
  revision: string;
  dtype: string;
  queries: Array<{ text: string; vector: number[] }>;
  chunks: Array<{ fingerprint: string }>;
  assets: Array<{ file: string; bytes: number; sha256: string }>;
};
export const runtimeAsset = {
  file: "libonnxruntime.so.1",
  bytes: 45828512,
  sha256: "ffb75a925ba05e47b235bb66e3e3911714a80b328a9c9425539feb204aa32a23",
};
const digest = (bytes: Buffer) =>
  createHash("sha256").update(bytes).digest("hex");

export async function qualifiedReference(): Promise<Reference> {
  expect(process.platform).toBe("linux");
  expect(process.arch).toBe("x64");
  expect([...new Set(cpus().map(({ model }) => model))]).toEqual([
    "AMD Ryzen 7 7800X3D 8-Core Processor",
  ]);
  const legacy = await readFile(
    new URL("./fixtures/search-semantic-model.json", import.meta.url),
  );
  expect(digest(legacy)).toBe(
    "fc735637f05dfd8aebce2ea75e1f5704f37248c4404ab40ef0117ded0dfcc133",
  );
  const linux = await readFile(
    new URL(
      "../../crates/lapis-app-core/tests/fixtures/search-model/linux-x64-ort-1.30.0-amd-7800x3d.json.txt",
      import.meta.url,
    ),
  );
  expect(digest(linux)).toBe(
    "4a5c17184d858df46f40ffdb34c78ebca1afd3dbb2327b599299efc38f13bbe6",
  );
  const fixture = JSON.parse(linux.toString());
  fixture.assets = JSON.parse(legacy.toString()).assets;
  return fixture;
}

export async function assertModel(database: AppDatabase, fixture: Reference) {
  for (const [index, row] of fixture.queries.entries()) {
    const record = await database.getSearchDocument(`${index}.md`);
    const embedding = record!.chunks![0].embedding!;
    expect(embedding.status).toBe("ready");
    expect(embedding.modelId).toBe(fixture.modelId);
    expect(embedding.dimensions).toBe(384);
    expect(embedding.vector).toHaveLength(384);
    expect(embedding.fingerprint).toBe(fixture.chunks[index].fingerprint);
    for (const [dimension, value] of row.vector.entries())
      expect(Math.abs(embedding.vector![dimension] - value)).toBeLessThan(
        0.00001,
      );
  }
  expect((await database.getSearchIndexStats()).readyChunkCount).toBe(6);
  const revision = await database.getChangeRevision();
  for (const row of fixture.queries) {
    const expected = fixture.queries
      .map((candidate, index) => ({
        path: `${index}.md`,
        score: candidate.vector.reduce(
          (sum, value, dimension) => sum + value * row.vector[dimension],
          0,
        ),
      }))
      .filter(({ score }) => score >= 0.2)
      .sort((a, b) => b.score - a.score);
    const results = await database.searchDocuments(row.text, {
      mode: "vector",
      includeDiagnostics: true,
    });
    expect(results.map(({ document }) => document.path)).toEqual(
      expected.map(({ path }) => path),
    );
    for (const [index, result] of results.entries()) {
      const vectorScore = result.scoreBreakdown?.vector;
      if (typeof vectorScore !== "number")
        throw new Error(
          "Native semantic results require a numeric vector score",
        );
      expect(Math.abs(vectorScore - expected[index].score)).toBeLessThan(
        0.0002,
      );
      expect(result.diagnostics).toMatchObject({
        providerKind: "transformers-js",
        appliedMode: "vector",
        vectorCandidateCount: expected.length,
      });
      expect(result.matchedChunkIds).toEqual([result.document.path + "#0"]);
      expect(result.snippets.length).toBeGreaterThan(0);
    }
  }
  expect(await database.getChangeRevision()).toBe(revision);
  expect(await database.getSearchEmbeddingRuntimeStatus()).toMatchObject({
    providerKind: "transformers-js",
    phase: "ready",
    modelId: fixture.modelId,
  });
}

export async function assertAssets(root: string, fixture: Reference) {
  const hashes: Array<[string, string]> = [];
  for (const asset of [...fixture.assets, runtimeAsset]) {
    const file = `${asset.sha256}-${basename(asset.file)}`;
    const bytes = await readFile(join(root, "profile/models", file));
    expect(bytes.length).toBe(asset.bytes);
    expect(digest(bytes)).toBe(asset.sha256);
    hashes.push([file, asset.sha256]);
  }
  return hashes;
}
