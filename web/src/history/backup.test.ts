import { afterEach, expect, it, vi } from "vitest";
import type { App } from "@lapis-notes/api";
import type {
  RepositoryBackupCommand,
  RepositoryBackupSnapshot,
} from "../../../crates/lapis-app-protocol/bindings/protocol";
import { createHostedBackup } from "./backup";

afterEach(() => vi.useRealTimers());
function fixture() {
  let enabled = true,
    version = "initial";
  let tail = Promise.resolve<unknown>(undefined);
  const actions: string[] = [];
  const enqueue = <T>(action: () => Promise<T>) => {
    const job = tail.then(action);
    tail = job.catch(() => {});
    return job;
  };
  const reload = vi.fn(async () => {
    actions.push("reload");
  });
  const app = {
    fileEditingSessions: {
      guard: vi.fn((_path: string, action: () => Promise<unknown>) =>
        enqueue(action),
      ),
      afterMutations: vi.fn((action: () => Promise<unknown>) =>
        enqueue(action),
      ),
    },
    configuration: { basePath: ".lapis/app.json", reloadFromDisk: reload },
  } as unknown as App;
  const call = vi.fn(
    async (
      command: RepositoryBackupCommand,
    ): Promise<RepositoryBackupSnapshot> => {
      actions.push(command.kind);
      if (command.kind === "set_enabled") {
        enabled = command.enabled;
        version = "changed";
      }
      return {
        enabled,
        configuration: { kind: "version", version },
        status: {
          phase: enabled ? "ready" : "closed",
          checkpoint: null,
          error: null,
        },
      };
    },
  );
  return {
    app,
    call,
    reload,
    actions,
    enqueue,
    service: createHostedBackup(app, call),
  };
}

it("start, periodic status and stop only observe native selection", async () => {
  vi.useFakeTimers();
  const f = fixture(),
    changed = vi.fn();
  await f.service.start(changed);
  await vi.advanceTimersByTimeAsync(2100);
  await f.service.stop();
  await vi.advanceTimersByTimeAsync(5000);
  expect(f.actions).toEqual(["status", "status", "status"]);
  expect(changed).toHaveBeenCalledTimes(3);
  expect(f.reload).not.toHaveBeenCalled();
});

it("queues conditional native selection and read-only reconciliation before later settings saves", async () => {
  const f = fixture();
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const preceding = f.enqueue(async () => {
    await gate;
    f.actions.push("prior-save");
  });
  const selecting = f.service.setEnabled!(false);
  expect(f.app.fileEditingSessions.guard).toHaveBeenCalledExactlyOnceWith(
    ".lapis/app.json",
    expect.any(Function),
  );
  expect(f.app.fileEditingSessions.afterMutations).toHaveBeenCalledOnce();
  const later = f.enqueue(async () => {
    f.actions.push("later-save");
  });
  release();
  await Promise.all([preceding, selecting, later]);
  expect(f.actions).toEqual([
    "prior-save",
    "status",
    "set_enabled",
    "reload",
    "later-save",
  ]);
  expect(f.call).toHaveBeenNthCalledWith(2, {
    kind: "set_enabled",
    enabled: false,
    expected: { kind: "version", version: "initial" },
  });
  expect(f.reload).toHaveBeenCalledExactlyOnceWith({
    materializeDefaults: false,
  });
});

it("reconciles a lost selection acknowledgement using passive status without replay", async () => {
  const f = fixture();
  const apply = f.call.getMockImplementation()!;
  f.call.mockImplementation(async (command) => {
    const result = await apply(command);
    if (command.kind === "set_enabled") throw new Error("lost acknowledgement");
    return result;
  });
  await expect(f.service.setEnabled!(false)).rejects.toThrow(
    "lost acknowledgement",
  );
  expect(f.actions).toEqual(["status", "set_enabled", "status", "reload"]);
  expect((await f.call({ kind: "status" })).enabled).toBe(false);
  expect(f.reload).toHaveBeenCalledOnce();
});

it("stop drains a pending observer and prevents its late callback or another poll", async () => {
  vi.useFakeTimers();
  const f = fixture(),
    changed = vi.fn();
  let deliver!: (value: RepositoryBackupSnapshot) => void;
  f.call.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        deliver = resolve;
      }),
  );
  const opening = f.service.start(changed);
  let stopped = false;
  const closing = f.service.stop().then(() => {
    stopped = true;
  });
  await Promise.resolve();
  expect(stopped).toBe(false);
  deliver({
    enabled: true,
    configuration: { kind: "missing" },
    status: { phase: "ready", checkpoint: null, error: null },
  });
  await Promise.all([opening, closing]);
  await vi.advanceTimersByTimeAsync(5000);
  expect(changed).not.toHaveBeenCalled();
  expect(f.call).toHaveBeenCalledOnce();
});

it("manual capture is explicit and does not persist a setting", async () => {
  const f = fixture();
  expect(await f.service.capture()).toEqual({
    phase: "ready",
    checkpoint: null,
    error: null,
  });
  expect(f.actions).toEqual(["capture"]);
});
