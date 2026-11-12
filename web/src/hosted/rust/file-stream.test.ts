import { expect, it, vi } from "vitest";
import { FileWatchHub, decodeFileFrame, readFileStream } from "./file-stream";
import { RustHttpClient } from "./client";
const encode = (
  event: string,
  revision: number,
  changes: unknown[] = [],
  reset = false,
) =>
  `event: ${event}\nid: ${revision}\ndata: ${JSON.stringify({ revision, changes, reset })}\n\n`;
const response = (body: BodyInit) =>
  new Response(body, { headers: { "content-type": "text/event-stream" } });

it("decodes split Unicode and CRLF frames without polling", async () => {
  const bytes = new TextEncoder().encode(
    encode("change", 4, [{ type: "create", path: "café.md" }]).replaceAll(
      "\n",
      "\r\n",
    ),
  );
  const stream = new ReadableStream({
    start(controller) {
      for (let i = 0; i < bytes.length; i += 3)
        controller.enqueue(bytes.slice(i, i + 3));
      controller.close();
    },
  });
  const accept = vi.fn();
  await readFileStream(response(stream), accept, new AbortController().signal);
  expect(accept).toHaveBeenCalledExactlyOnceWith({
    event: "change",
    value: {
      revision: 4,
      changes: [{ type: "create", path: "café.md" }],
      reset: false,
    },
  });
});
it("rejects unsafe paths, malformed IDs, oversized and truncated frames", async () => {
  const accept = vi.fn();
  for (const body of [
    encode("change", 1, [{ type: "create", path: "../escape" }]),
    encode("change", 1, [
      { type: "rename", path: "new.md", previous_path: "/private" },
    ]),
    encode("other", 1),
    encode("change", 1).replace("id: 1", "id: 2"),
    "x".repeat(128 * 1024 + 1),
    encode("ready", 0).trim(),
    new Uint8Array([0xff]),
  ]) {
    await expect(
      readFileStream(response(body), accept, new AbortController().signal),
    ).rejects.toThrow();
  }
  expect(accept).not.toHaveBeenCalled();
});
it("abort cancels an idle response and releases the reader", async () => {
  const cancelled = vi.fn();
  const r = response(new ReadableStream({ cancel: cancelled }));
  const controller = new AbortController();
  const reading = readFileStream(r, vi.fn(), controller.signal);
  controller.abort();
  await reading;
  expect(cancelled).toHaveBeenCalledOnce();
  expect(r.body?.locked).toBe(false);
});
function fixture() {
  const controllers: ReadableStreamDefaultController<Uint8Array>[] = [];
  const fetch = vi.fn<typeof globalThis.fetch>(async (url, init) => {
    if (String(url).includes("/file-events?"))
      return response(
        new ReadableStream({
          start(controller) {
            controllers.push(controller);
          },
        }),
      );
    if (init?.method === "DELETE") return new Response(null, { status: 204 });
    return Response.json({
      client_id: "client",
      vault_id: "vault",
      generation: "generation",
    });
  });
  return {
    controllers,
    fetch,
    push: (value: string, index = 0) =>
      controllers[index].enqueue(new TextEncoder().encode(value)),
  };
}
it("shares one stream, honors watch scopes, and waits for native registration", async () => {
  const f = fixture(),
    client = await new RustHttpClient("", f.fetch).attach("vault");
  const hub = new FileWatchHub(client);
  const notes = vi.fn(),
    config = vi.fn();
  const first = hub.watch("Notes", { recursive: true }, notes);
  const second = hub.watch(".lapis/app.json", {}, config);
  try {
    await vi.waitFor(() => expect(f.controllers).toHaveLength(1));
    let ready = false;
    void first.ready?.then(() => {
      ready = true;
    });
    await Promise.resolve();
    expect(ready).toBe(false);
    f.push(encode("ready", 0));
    await Promise.all([first.ready, second.ready]);
    f.push(
      encode("change", 1, [
        { type: "create", path: "Notes/file.md" },
        { type: "modify", path: ".lapis/app.json" },
        { type: "create", path: "Other.md" },
      ]),
    );
    await vi.waitFor(() => expect(notes).toHaveBeenCalledOnce());
    expect(config).toHaveBeenCalledExactlyOnceWith({
      type: "modify",
      path: ".lapis/app.json",
    });
    await first.close();
    f.push(encode("change", 2, [{ type: "modify", path: "Notes/file.md" }]));
    await Promise.resolve();
    expect(notes).toHaveBeenCalledOnce();
  } finally {
    await second.close();
    await client.close();
  }
});
it("reconnects with a replay cursor and stops on auth failure", async () => {
  vi.useFakeTimers();
  let requests = 0;
  const fetch = vi.fn<typeof globalThis.fetch>(async (url, init) => {
    if (String(url).includes("/file-events?")) {
      if (++requests === 1)
        return response(
          encode("ready", 0) +
            encode("change", 4, [{ type: "create", path: "Note.md" }]),
        );
      return Response.json({ code: "PERMISSION_DENIED" }, { status: 401 });
    }
    if (init?.method === "DELETE") return new Response(null, { status: 204 });
    return Response.json({
      client_id: "client",
      vault_id: "vault",
      generation: "generation",
    });
  });
  const client = await new RustHttpClient("", fetch).attach("vault");
  const accept = vi.fn();
  const watch = new FileWatchHub(client).watch(
    "/",
    { recursive: true },
    accept,
  );
  try {
    await watch.ready;
    await vi.advanceTimersByTimeAsync(501);
    expect(requests).toBe(2);
    expect(
      String(
        fetch.mock.calls.find(([u]) => String(u).includes("cursor="))?.[0],
      ),
    ).toContain("cursor=4");
    await vi.advanceTimersByTimeAsync(5000);
    expect(requests).toBe(2);
    expect(accept.mock.calls.some(([e]) => e.type === "error")).toBe(true);
  } finally {
    await watch.close();
    await client.close();
    vi.useRealTimers();
  }
});
it("a new subscription can start while the previous stream is retiring", async () => {
  const f = fixture(),
    client = await new RustHttpClient("", f.fetch).attach("vault"),
    hub = new FileWatchHub(client);
  const first = hub.watch("/", { recursive: true }, vi.fn());
  await vi.waitFor(() => expect(f.controllers).toHaveLength(1));
  f.push(encode("ready", 0));
  await first.ready;
  const retiring = first.close();
  const second = hub.watch("/", { recursive: true }, vi.fn());
  try {
    await retiring;
    await vi.waitFor(() => expect(f.controllers).toHaveLength(2));
    f.push(encode("ready", 0), 1);
    await second.ready;
  } finally {
    await second.close();
    await client.close();
  }
});

it("reconnects after a stalled transport deadline without forcing a tree scan", async () => {
  vi.useFakeTimers();
  const f = fixture();
  const ordinaryFetch = f.fetch.getMockImplementation()!;
  let attempts = 0;
  f.fetch.mockImplementation(async (url, init) => {
    if (String(url).includes("/file-events?") && ++attempts === 1)
      throw new DOMException("Stream deadline", "TimeoutError");
    return ordinaryFetch(url, init);
  });
  const client = await new RustHttpClient("", f.fetch).attach("vault");
  const accept = vi.fn();
  const watch = new FileWatchHub(client).watch(
    "/",
    { recursive: true },
    accept,
  );
  try {
    await vi.advanceTimersByTimeAsync(501);
    expect(attempts).toBe(2);
    f.push(encode("ready", 0));
    await watch.ready;
    expect(accept).not.toHaveBeenCalled();
  } finally {
    await watch.close();
    await client.close();
    vi.useRealTimers();
  }
});
