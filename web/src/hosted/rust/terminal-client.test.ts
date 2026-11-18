import { expect, it, vi } from "vitest";
import { createHostedTerminal } from "./terminal-client";
import type { RustVaultClient } from "./vault-client";
import { readTerminalStream } from "./terminal-stream";

type TerminalReply = { kind: string; session_id?: string };

function fixture() {
  let trusted = true;
  const abort = new AbortController();
  const json = vi.fn(
    async (
      _path: string,
      _method: string,
      command: { kind: string },
    ): Promise<TerminalReply> =>
      command.kind === "create"
        ? { kind: "created", session_id: "session" }
        : { kind: "done" },
  );
  const response = vi.fn(
    async (_path: string, _options?: RequestInit) =>
      new Response(new ReadableStream({ start() {} }), {
        headers: { "content-type": "text/event-stream" },
      }),
  );
  const client = {
    path: "/clients/owner",
    signal: abort.signal,
    requireOpen: vi.fn(),
    host: { json, response },
  } as unknown as RustVaultClient;
  const terminal = createHostedTerminal(client, () => trusted, vi.fn());
  return {
    terminal,
    json,
    response,
    trust: (value: boolean) => {
      trusted = value;
    },
  };
}

it("gates shell creation by trust and sends only admitted portable options", async () => {
  const f = fixture();
  f.trust(false);
  await expect(f.terminal.sessions.create()).rejects.toThrow(/trusted/);
  expect(f.json).not.toHaveBeenCalled();
  f.trust(true);
  await expect(
    f.terminal.sessions.create({ shell: "/bin/zsh" }),
  ).rejects.toThrow(/host shell/);
  expect(
    await f.terminal.sessions.create({ cwd: "notes", cols: 90, rows: 25 }),
  ).toEqual({ sessionId: "session", status: "running" });
  expect(f.json).toHaveBeenCalledWith("/clients/owner/terminal", "POST", {
    kind: "create",
    cwd: "notes",
    cols: 90,
    rows: 25,
  });
  await f.terminal.closeAll();
  expect(f.response.mock.calls).toHaveLength(1);
  await f.terminal.sessions.create();
  await f.terminal.dispose();
  expect(f.terminal.sessions.available).toBe(false);
});

it("decodes split raw-byte output frames and rejects oversized byte arrays", async () => {
  const bytes = new TextEncoder();
  const frame =
    "event: output\ndata: " +
    JSON.stringify({
      cursor: 3,
      bytes: [0, 128, 255],
      running: false,
      truncated: false,
      exit_code: 0,
    }) +
    "\n\n";
  const accept = vi.fn();
  await readTerminalStream(
    new Response(
      new ReadableStream({
        start(controller) {
          controller.enqueue(bytes.encode(frame.slice(0, 30)));
          controller.enqueue(bytes.encode(frame.slice(30)));
          controller.close();
        },
      }),
      { headers: { "content-type": "text/event-stream" } },
    ),
    new AbortController().signal,
    accept,
  );
  expect(accept).toHaveBeenCalledWith({
    cursor: 3,
    bytes: [0, 128, 255],
    running: false,
    truncated: false,
    exit_code: 0,
  });
  const bad = frame.replace("[0,128,255]", "[256]");
  await expect(
    readTerminalStream(
      new Response(bad, { headers: { "content-type": "text/event-stream" } }),
      new AbortController().signal,
      accept,
    ),
  ).rejects.toThrow(/Invalid terminal/);
});

it("stops a creation that finishes after admission is revoked", async () => {
  const f = fixture();
  let resolve!: (value: TerminalReply) => void;
  f.json.mockImplementationOnce(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const creating = f.terminal.sessions.create();
  f.trust(false);
  const closing = f.terminal.closeAll();
  resolve({ kind: "created", session_id: "late" });
  await expect(creating).rejects.toThrow(/admission changed/);
  await closing;
  expect(f.json).toHaveBeenCalledWith("/clients/owner/terminal", "POST", {
    kind: "stop",
    session_id: "late",
  });
  await f.terminal.dispose();
});

it("preserves input order while earlier HTTP writes are pending", async () => {
  const f = fixture();
  await f.terminal.sessions.create();
  let release!: (value: TerminalReply) => void;
  f.json.mockImplementationOnce(
    () =>
      new Promise((done) => {
        release = done;
      }),
  );
  const first = f.terminal.sessions.write("session", "first");
  const second = f.terminal.sessions.write("session", "second");
  await Promise.resolve();
  expect(
    f.json.mock.calls.filter(([, , command]) => command.kind === "write"),
  ).toHaveLength(1);
  release({ kind: "done" });
  await Promise.all([first, second]);
  expect(
    f.json.mock.calls
      .filter(([, , command]) => command.kind === "write")
      .map(([, , command]) => (command as { data?: string }).data),
  ).toEqual(["first", "second"]);
  await f.terminal.dispose();
});

it("reports an older server without a Terminal route as unavailable", async () => {
  const { RustHostError } = await import("./client");
  const f = fixture();
  f.json.mockRejectedValueOnce(
    new RustHostError("IO", "Server request failed (404)", 404),
  );
  await expect(f.terminal.sessions.create()).rejects.toThrow();
  expect(f.terminal.sessions.available).toBe(false);
  expect(f.terminal.sessions.reason).toContain("Update the server");
  await f.terminal.dispose();
});

it("can retire an older server client before any Terminal is opened", async () => {
  const { RustHostError } = await import("./client");
  const f = fixture();
  f.json.mockRejectedValueOnce(
    new RustHostError("IO", "Server request failed (404)", 404),
  );
  await expect(f.terminal.dispose()).resolves.toBeUndefined();
});

it("resumes finite output responses without replaying earlier bytes", async () => {
  const f = fixture();
  const output = vi.fn();
  f.terminal.sessions.onOutput(output);
  f.response.mockResolvedValueOnce(
    new Response(
      'event: output\ndata: {"cursor":3,"bytes":[97,98,99],"running":true,"truncated":false,"exit_code":null}\n\n',
      { headers: { "content-type": "text/event-stream" } },
    ),
  );
  await f.terminal.sessions.create();
  await vi.waitFor(() => expect(f.response).toHaveBeenCalledTimes(2));
  expect(f.response.mock.calls[1][0]).toBe(
    "/clients/owner/terminal/session/events?cursor=3",
  );
  expect(output).toHaveBeenCalledOnce();
  await f.terminal.dispose();
});
