import { expect, it, vi } from "vitest";
import { RustHttpClient } from "./client";

const admission = {
  client_id: "client/id",
  vault_id: "vault/id",
  generation: "gen/id",
};
async function fixture() {
  const fetch = vi.fn<typeof globalThis.fetch>(async (_url, init) => {
    if (init?.method === "DELETE") return new Response(null, { status: 204 });
    return Response.json(admission);
  });
  const host = new RustHttpClient("", fetch);
  const client = await host.attach(admission.vault_id);
  return { client, fetch };
}
const path =
  "/api/v3/clients/client%2Fid/repository-remotes?vault=vault%2Fid&generation=gen%2Fid";
it("scopes remote reads and exact commands to the admitted vault and generation", async () => {
  const { client, fetch } = await fixture();
  try {
    const snapshot = { revision: 7 };
    fetch.mockResolvedValueOnce(Response.json(snapshot));
    expect(await client.repositoryRemotes()).toEqual(snapshot);
    const command = {
      kind: "push" as const,
      target: "original" as const,
      expectedRevision: 7,
      reviewId: "stable-review",
    };
    fetch.mockResolvedValueOnce(Response.json(snapshot));
    await client.repositoryRemotes(command);
    expect(
      fetch.mock.calls
        .slice(1)
        .map(([url, init]) => [url, init?.method, init?.body]),
    ).toEqual([
      [path, "GET", undefined],
      [path, "POST", JSON.stringify(command)],
    ]);
  } finally {
    await client.close();
  }
});
it("fences a lost remote reply after client release without replaying publication", async () => {
  const { client, fetch } = await fixture();
  try {
    let deliver!: (response: Response) => void;
    fetch.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          deliver = resolve;
        }),
    );
    const pending = client.repositoryRemotes({
      kind: "push",
      target: "backup",
      expectedRevision: 7,
      reviewId: "stable-review",
    });
    const failure = expect(pending).rejects.toThrow("closed");
    await vi.waitFor(() => expect(deliver).toBeTypeOf("function"));
    await client.close();
    deliver(Response.json({ revision: 8 }));
    await failure;
    await expect(client.repositoryRemotes()).rejects.toThrow("closed");
    expect(
      fetch.mock.calls.filter(([url]) => String(url) === path),
    ).toHaveLength(1);
  } finally {
    await client.close();
  }
});
it("preserves raw working-copy observations and refuses stale delivery", async () => {
  const { client, fetch } = await fixture();
  try {
    const text =
      '{"version":1,"operation":"exact","commit":{"author":{"timestamp":{"millis":"18446744073709551615"}}}}';
    fetch.mockResolvedValueOnce(new Response(text));
    expect(await client.repositoryWorkingCopy()).toBe(text);
    expect(fetch.mock.calls[1][0]).toBe(
      path.replace("repository-remotes", "repository-working-copy"),
    );
    await client.close();
    await expect(client.repositoryWorkingCopy()).rejects.toThrow("closed");
    expect(fetch.mock.calls).toHaveLength(3);
  } finally {
    await client.close();
  }
});
