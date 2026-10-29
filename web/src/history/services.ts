import type { HistoryClient, Handle } from "@lapismd/jj-history";
import { PreparationQueue } from "./preparation-queue";

export type BackupCheckpoint = {
  operation: string;
  commit: string;
  change_id: string;
  description: string;
  timestamp_millis: number;
};

export type BackupStatus = {
  phase: "saving" | "ready" | "failed" | "closed";
  checkpoint: BackupCheckpoint | null;
  error: string | null;
};

export interface BackupService {
  setEnabled?(enabled: boolean): Promise<void>;
  start(changed: (status: BackupStatus) => void): Promise<BackupStatus>;
  capture(): Promise<BackupStatus>;
  stop(): Promise<void>;
}

export type RestorePreview = {
  checkpoint: string;
  path: string;
  expected: string;
  bytes: number;
  deleted: boolean;
};

export type VaultRestorePreview = {
  checkpoint: string;
  expected: string;
  added: number;
  modified: number;
  deleted: number;
};

export type FileRestoreRequest = {
  request_id: string;
  checkpoint: string;
  path: string;
  expected: string;
};

export type VaultRestoreRequest = {
  request_id: string;
  checkpoint: string;
  expected: string;
};

export interface RestoreService {
  preview(checkpoint: string, path: string): Promise<RestorePreview>;
  apply(request: FileRestoreRequest): Promise<unknown>;
  previewVault?(checkpoint: string): Promise<VaultRestorePreview>;
  applyVault?(request: VaultRestoreRequest): Promise<unknown>;
  keepCurrent?(): Promise<unknown>;
}

type RepositoryReader = Pick<
  HistoryClient,
  "heads" | "open" | "status" | "read" | "blame" | "close"
>;
export type RepositoryServices = {
  reader: RepositoryReader;
  backup?: BackupService;
  restore?: RestoreService;
  sources?: () => Promise<{ original: boolean }>;
  original?: RepositoryReader;
  close(mode?: string): Promise<void>;
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
