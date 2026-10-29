import { describe, expect, it, vi } from "vitest";
import { RustHttpClient } from "./client";
import { RustVaultAdapter } from "./vault-adapter";

describe("Rust vault adapter", () => {
  it.each(["/", ".lapis"])(
    "retains idempotent directory creation for %s",
    async (path) => {
      const fetch = vi.fn<typeof globalThis.fetch>(async (_url, init) => {
        if (init?.method === "DELETE")
          return new Response(null, { status: 204 });
        const request = JSON.parse(String(init?.body));
        if (!request.command)
          return Response.json({
            client_id: "client",
            vault_id: "vault",
            generation: "generation",
          });
        if (request.command.operation.operation === "mkdir")
          return Response.json(
            { code: "ALREADY_EXISTS", message: "File already exists" },
            { status: 409 },
          );
        return Response.json({
          protocol_version: 3,
          request_id: request.request_id,
          outcome: {
            status: "ok",
            value: {
              kind: "files",
              value: {
                kind: "stat",
                stat: { type: "folder", size: 0, ctime: 1, mtime: 1 },
              },
            },
          },
        });
      });
      const client = await new RustHttpClient("", fetch).attach("vault");
      try {
        await expect(
          new RustVaultAdapter(client, "Vault").mkdir(path),
        ).resolves.toBeUndefined();
        const requests = fetch.mock.calls
          .slice(1)
          .map(([, init]) => JSON.parse(String(init?.body)));
        expect(
          requests.map((request) => request.command.operation.operation),
        ).toEqual(["mkdir", "stat"]);
        expect(requests[0].command.operation.path).toBe(
          path === "/" ? "" : path,
        );
      } finally {
        await client.close();
      }
    },
  );
});
