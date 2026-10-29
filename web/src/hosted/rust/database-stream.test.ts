import { expect, it, vi } from "vitest";
import { observeDatabaseChanges, readDatabaseStream } from "./database-stream";
import { RustHttpClient } from "./client";

const change = {
  revision: 4,
  domains: ["metadata"],
  paths: ["café.md"],
  committedAt: 12,
};

it("reads bounded split UTF-8 change frames and ignores heartbeat comments", async () => {
  const bytes = new TextEncoder().encode(
    `: keep-alive\r\n\r\nevent: change\r\nid: 4\r\ndata: ${JSON.stringify(change)}\r\n\r\n`,
  );
  const response = new Response(
    new ReadableStream({
      start(controller) {
        for (let i = 0; i < bytes.length; i += 7)
          controller.enqueue(bytes.slice(i, i + 7));
        controller.close();
      },
    }),
    { headers: { "content-type": "text/event-stream" } },
  );
  const accept = vi.fn();
  await readDatabaseStream(response, accept, new AbortController().signal);
  expect(accept).toHaveBeenCalledExactlyOnceWith(change);
});

it("rejects malformed, oversized, unmatched and out-of-contract frames", async () => {
  for (const value of [
    `event: change\nid: 3\ndata: ${JSON.stringify(change)}\n\n`,
    `event: change\nid: 4\ndata: ${JSON.stringify({ ...change, domains: ["unknown"] })}\n\n`,
    `event: change\nid: 4\ndata: ${JSON.stringify({ ...change, revision: -1 })}\n\n`,
    `event: change\nid: 4\ndata: invalid\n\n`,
    "x".repeat(256 * 1024 + 1),
  ]) {
    const accept = vi.fn();
    await expect(
      readDatabaseStream(
        new Response(value, {
          headers: { "content-type": "text/event-stream" },
        }),
        accept,
        new AbortController().signal,
      ),
    ).rejects.toThrow();
    expect(accept).not.toHaveBeenCalled();
  }
});

it("abort cancels an idle reader and releases its stream without a mutation", async () => {
  const cancelled = vi.fn();
  const response = new Response(new ReadableStream({ cancel: cancelled }), {
    headers: { "content-type": "text/event-stream" },
  });
  const controller = new AbortController();
  const reading = readDatabaseStream(response, vi.fn(), controller.signal);
  controller.abort();
  await reading;
  expect(cancelled).toHaveBeenCalledOnce();
  expect(response.body?.locked).toBe(false);
});

it("reconnects from its cursor and stops permanently after authentication rejection", async () => {
  vi.useFakeTimers();
  const admission = {
    client_id: "client",
    vault_id: "vault",
    generation: "generation",
  };
  let requests = 0;
  const fetch = vi.fn<typeof globalThis.fetch>(async (url, init) => {
    if (init?.method === "DELETE") return new Response(null, { status: 204 });
    if (!String(url).includes("/database/events?"))
      return Response.json(admission);
    if (++requests === 1)
      return new Response(
        `event: change\nid: 4\ndata: ${JSON.stringify(change)}\n\n`,
        { headers: { "content-type": "text/event-stream" } },
      );
    return Response.json({ code: "PERMISSION_DENIED" }, { status: 401 });
  });
  const client = await new RustHttpClient("", fetch).attach("vault");
  const accept = vi.fn(),
    report = vi.fn();
  const observer = observeDatabaseChanges(client, accept, report);
  try {
    await vi.advanceTimersByTimeAsync(0);
    expect(accept).toHaveBeenCalledOnce();
    await vi.advanceTimersByTimeAsync(1000);
    await observer.done;
    const reads = fetch.mock.calls.filter(([url]) =>
      String(url).includes("/database/events?"),
    );
    expect(reads).toHaveLength(2);
    expect(String(reads[1][0])).toContain("cursor=4");
    expect(report).toHaveBeenCalledExactlyOnceWith(
      expect.objectContaining({ status: 401 }),
    );
    await vi.advanceTimersByTimeAsync(5000);
    expect(requests).toBe(2);
  } finally {
    observer.stop();
    await observer.done;
    await client.close();
    vi.useRealTimers();
  }
});

it("closing the admitted client aborts an idle observation immediately", async () => {
  const cancelled = vi.fn();
  const admission = {
    client_id: "client",
    vault_id: "vault",
    generation: "generation",
  };
  const fetch = vi.fn<typeof globalThis.fetch>(async (url, init) => {
    if (init?.method === "DELETE") return new Response(null, { status: 204 });
    if (String(url).includes("/database/events?"))
      return new Response(new ReadableStream({ cancel: cancelled }), {
        headers: { "content-type": "text/event-stream" },
      });
    return Response.json(admission);
  });
  const client = await new RustHttpClient("", fetch).attach("vault");
  const report = vi.fn();
  const observer = observeDatabaseChanges(client, vi.fn(), report);
  try {
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(2));
    await client.close();
    await observer.done;
    expect(cancelled).toHaveBeenCalledOnce();
    expect(report).not.toHaveBeenCalled();
  } finally {
    observer.stop();
    await observer.done;
    await client.close();
  }
});
