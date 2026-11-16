import type { HistoryClient, Handle } from "@lapismd/jj-history";
import { PreparationQueue } from "./preparation-queue";

import type {
  RepositoryServices,
  RepositoryRemotesService,
} from "@lapis-notes/history/repository";
export type { RepositoryServices } from "@lapis-notes/history/repository";
export type BackupService = NonNullable<RepositoryServices["backup"]>;
export type BackupStatus = Awaited<ReturnType<BackupService["capture"]>>;
export type BackupCheckpoint = NonNullable<BackupStatus["checkpoint"]>;
export type RestoreService = NonNullable<RepositoryServices["restore"]>;
export type RestorePreview = Awaited<ReturnType<RestoreService["preview"]>>;
export type FileRestoreRequest = Parameters<RestoreService["apply"]>[0];
export type VaultRestorePreview = Awaited<
  ReturnType<NonNullable<RestoreService["previewVault"]>>
>;
export type VaultRestoreRequest = Parameters<
  NonNullable<RestoreService["applyVault"]>
>[0];
type RemotePorts = {
  remotes: RepositoryRemotesService & { close(): void };
  workingCopy: NonNullable<RepositoryServices["workingCopy"]>;
};

type OriginalSource = {
  discover: () => Promise<{ original: boolean }>;
  open: () => Promise<HistoryClient>;
};

/** Owns this workspace's reads only. Capture and indexing belong to the native vault owner. */
export function createHostedRepository(
  reader: HistoryClient,
  restore?: RestoreService,
  backup?: BackupService,
  source?: OriginalSource,
  remote?: RemotePorts,
): RepositoryServices {
  const preparations = new PreparationQueue(reader);
  const pending = new Set<Promise<unknown>>();
  const handles = new Map<Handle, HistoryClient>();
  let original: HistoryClient | undefined;
  let originalActivation: Promise<void> | undefined;
  let closed = false;
  let closing: Promise<void> | undefined;
  function track<T>(action: () => Promise<T>, immediate = false): Promise<T> {
    if (closed)
      return Promise.reject(new Error("Repository history is closed"));
    const job = immediate ? action() : Promise.resolve().then(action);
    pending.add(job);
    void job.finally(() => pending.delete(job)).catch(() => {});
    return job;
  }
  function observe<T>(action: () => Promise<T>): Promise<T> {
    return track(async () => {
      const result = await action();
      if (closed) throw new Error("Repository history is closed");
      return result;
    });
  }
  async function closeHandle(handle: Handle, current: HistoryClient) {
    await preparations.close(handle, current);
    handles.delete(handle);
  }
  function originalRun<T>(action: () => Promise<T>): Promise<T> {
    return track(async () => {
      originalActivation ??= source!
        .open()
        .then((reader) => {
          original = reader;
        })
        .catch((error) => {
          originalActivation = undefined;
          throw error;
        });
      await originalActivation;
      if (closed) throw new Error("Repository history is closed");
      return action();
    });
  }
  function scoped(
    current: () => HistoryClient,
    execute: typeof track,
  ): RepositoryServices["reader"] {
    return {
      heads: () => execute(() => current().heads()),
      open: (operation) =>
        execute(async () => {
          const reader = current();
          const handle = await preparations.open(operation, reader);
          handles.set(handle, reader);
          return handle;
        }),
      status: (handle) => execute(() => current().status(handle)),
      read: (handle, query) => execute(() => current().read(handle, query)),
      blame: (handle, query) => execute(() => current().blame(handle, query)),
      close: (handle) => execute(() => closeHandle(handle, current())),
    };
  }
  return {
    remotes: remote
      ? {
          synchronization: remote.remotes.synchronization
            ? {
                read: () =>
                  observe(() => remote.remotes.synchronization!.read()),
                execute: (command) =>
                  observe(() =>
                    remote.remotes.synchronization!.execute(command),
                  ),
              }
            : undefined,
          read: () => observe(() => remote.remotes.read()),
          execute: (command) => observe(() => remote.remotes.execute(command)),
          openSignIn: () => observe(() => remote.remotes.openSignIn()),
        }
      : undefined,
    workingCopy: remote ? () => observe(remote.workingCopy) : undefined,
    backup: backup
      ? {
          start: (changed) => track(() => backup.start(changed)),
          stop: () => track(() => backup.stop()),
          capture: () => track(() => backup.capture()),
          ...(backup.setEnabled
            ? {
                setEnabled: (enabled: boolean) =>
                  track(() => backup.setEnabled!(enabled), true),
              }
            : {}),
        }
      : undefined,
    restore: restore
      ? {
          preview: (checkpoint, path) =>
            track(() => restore.preview(checkpoint, path)),
          apply: (request) => track(() => restore.apply(request)),
          ...(restore.previewVault
            ? {
                previewVault: (checkpoint: string) =>
                  track(() => restore.previewVault!(checkpoint)),
              }
            : {}),
          ...(restore.applyVault
            ? {
                applyVault: (
                  request: Parameters<
                    NonNullable<RestoreService["applyVault"]>
                  >[0],
                ) => track(() => restore.applyVault!(request)),
              }
            : {}),
          ...(restore.keepCurrent
            ? { keepCurrent: () => track(() => restore.keepCurrent!()) }
            : {}),
        }
      : undefined,
    sources: source ? () => track(() => source.discover()) : undefined,
    original: source ? scoped(() => original!, originalRun) : undefined,
    reader: scoped(() => reader, track),
    close() {
      closed = true;
      remote?.remotes.close();
      preparations.stop();
      return (closing ??= (async () => {
        while (pending.size) await Promise.allSettled([...pending]);
        await backup?.stop();
        const results = await Promise.allSettled(
          [...handles].map(([handle, reader]) => closeHandle(handle, reader)),
        );
        const failure = results.find((result) => result.status === "rejected");
        if (failure?.status === "rejected") throw failure.reason;
      })().catch((error) => {
        closing = undefined;
        throw error;
      }));
    },
  };
}
