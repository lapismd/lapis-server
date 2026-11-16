import { expect, it, vi } from "vitest";
import type {
  RepositoryRemotesCommand,
  RepositoryRemotesSnapshot,
} from "../../../crates/lapis-app-protocol/bindings/protocol";
import { createHostedRemotes } from "./remotes";

function fixture() {
  const snapshot: RepositoryRemotesSnapshot = {
    revision: 17,
    capabilities: {
      targets: ["backup", "original"],
      anonymousHttps: true,
      httpsTokens: true,
      hostSshAgent: true,
      hostSshDefault: true,
      githubDeviceSignIn: true,
      forgejoTokens: true,
    },
    settings: {
      host: "server",
      storage: "available",
      sshAvailable: true,
      deviceSignInAvailable: true,
      credentials: [],
      targets: [],
      automaticSync: false,
      authorization: {
        code: "CURRENT-CODE",
        url: "https://github.com/login/device",
        state: "pending",
      },
    },
  };
  const call = vi.fn(async (_command?: RepositoryRemotesCommand) => snapshot);
  const lifetime = new AbortController();
  const events: string[] = [];
  const copy = vi.fn(async (value: string) => {
    events.push(`copy:${value}`);
  });
  const open = vi.fn((url: string) => {
    events.push(`open:${url}`);
  });
  const service = createHostedRemotes(call, lifetime.signal, { copy, open });
  return { snapshot, call, lifetime, events, copy, open, service };
}

it("retains the native snapshot and exact target/review command without retries", async () => {
  const f = fixture();
  expect(await f.service.read()).toBe(f.snapshot);
  const command = {
    kind: "push",
    expectedRevision: 17,
    target: "original",
    reviewId: "immutable-review",
  } as const;
  expect(await f.service.execute(command)).toBe(f.snapshot);
  expect(f.call.mock.calls).toEqual([[undefined], [command]]);
  f.call.mockRejectedValueOnce(new Error("Lost acknowledgement"));
  await expect(f.service.execute(command)).rejects.toThrow(
    "Lost acknowledgement",
  );
  expect(f.call).toHaveBeenCalledTimes(3);
});

it.each(["github.com", "github.enterprise.example"])(
  "copies the current device code before opening its native-approved HTTPS page on %s",
  async (host) => {
    const f = fixture();
    f.snapshot.settings.authorization!.url = `https://${host}/login/device`;
    await f.service.read();
    f.snapshot.settings.authorization!.code = "NEW-CODE";
    await f.service.openSignIn();
    expect(f.call).toHaveBeenCalledTimes(2);
    expect(f.events).toEqual([
      "copy:NEW-CODE",
      `open:https://${host}/login/device`,
    ]);
  },
);

it.each([undefined, "expired"] as const)(
  "refuses an unavailable device authorization: %s",
  async (state) => {
    const f = fixture();
    if (state) f.snapshot.settings.authorization!.state = state;
    else delete f.snapshot.settings.authorization;
    await expect(f.service.openSignIn()).rejects.toThrow("Sign in again");
    expect(f.events).toEqual([]);
  },
);

it.each([
  "http://github.com/login/device",
  "https://token@github.com/login/device",
  "https://github.com/login/device?code=secret",
  "https://github.com/login/device#secret",
  "https://github.com/other",
  "javascript:alert(1)",
])("rejects an unsafe verification URL without copying: %s", async (url) => {
  const f = fixture();
  f.snapshot.settings.authorization!.url = url;
  await expect(f.service.openSignIn()).rejects.toThrow("verification page");
  expect(f.events).toEqual([]);
});

it("does not open the verification page if clipboard delivery fails", async () => {
  const f = fixture();
  f.copy.mockRejectedValueOnce(new Error("Clipboard unavailable"));
  await expect(f.service.openSignIn()).rejects.toThrow("Clipboard unavailable");
  expect(f.open).not.toHaveBeenCalled();
});

it("fences late snapshots and new commands after client detach", async () => {
  const f = fixture();
  f.call.mockImplementationOnce(async () => {
    f.lifetime.abort();
    return f.snapshot;
  });
  await expect(f.service.openSignIn()).rejects.toThrow("closed");
  await expect(f.service.read()).rejects.toThrow("closed");
  await expect(
    f.service.execute({
      kind: "fetch",
      expectedRevision: 17,
      target: "backup",
    }),
  ).rejects.toThrow("closed");
  expect(f.call).toHaveBeenCalledOnce();
  expect(f.events).toEqual([]);
});

it("does not open a page after detach during clipboard delivery", async () => {
  const f = fixture();
  f.copy.mockImplementationOnce(async () => {
    f.lifetime.abort();
  });
  await expect(f.service.openSignIn()).rejects.toThrow("closed");
  expect(f.open).not.toHaveBeenCalled();
});

it("retires the repository observer without aborting accepted native work", async () => {
  const f = fixture();
  let deliver!: (snapshot: RepositoryRemotesSnapshot) => void;
  f.call.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        deliver = resolve;
      }),
  );
  const response = f.service.execute({
    kind: "fetch",
    expectedRevision: 17,
    target: "backup",
  });
  const refused = expect(response).rejects.toThrow("closed");
  f.service.close();
  expect(f.lifetime.signal.aborted).toBe(false);
  deliver(f.snapshot);
  await refused;
  await expect(f.service.openSignIn()).rejects.toThrow("closed");
  expect(f.events).toEqual([]);
});
