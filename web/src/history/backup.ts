import type { App } from "@lapis-notes/api";
import type {
  RepositoryBackupCommand,
  RepositoryBackupSnapshot,
} from "../../../crates/lapis-app-protocol/bindings/protocol";
import type { BackupService, BackupStatus } from "./services";

/** Views observe the selected native owner; only an explicit setting persists. */
export function createHostedBackup(
  app: App,
  call: (command: RepositoryBackupCommand) => Promise<RepositoryBackupSnapshot>,
): BackupService {
  let epoch = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let observation: Promise<BackupStatus> | undefined;
  let latest: BackupStatus = { phase: "closed", checkpoint: null, error: null };
  const read = async () => {
    const result = await call({ kind: "status" });
    if (
      !result ||
      typeof result.enabled !== "boolean" ||
      !result.status ||
      !["saving", "ready", "closed", "failed"].includes(result.status.phase)
    )
      throw new Error("Invalid native backup status");
    return result.status;
  };
  return {
    async start(changed) {
      clearTimeout(timer);
      const current = ++epoch;
      const poll = async (): Promise<BackupStatus> => {
        const job = read();
        observation = job;
        try {
          const status = await job;
          if (epoch === current) {
            latest = status;
            changed(status);
          }
          return status;
        } finally {
          if (observation === job) observation = undefined;
        }
      };
      const schedule = () => {
        if (epoch !== current) return;
        timer = setTimeout(() => {
          void poll()
            .catch((error) => {
              if (epoch === current)
                changed({ ...latest, phase: "failed", error: String(error) });
            })
            .finally(schedule);
        }, 1000);
      };
      const initial = await poll();
      schedule();
      return initial;
    },
    async stop() {
      epoch++;
      clearTimeout(timer);
      if (observation) await Promise.allSettled([observation]);
    },
    async capture() {
      return (await call({ kind: "capture" })).status;
    },
    async setEnabled(enabled) {
      // Queue both before yielding. Later config saves must calculate from
      // the refreshed baseline, without a nested editing-registry guard.
      const mutation = app.fileEditingSessions.guard(
        app.configuration.basePath,
        async () => {
          const before = await call({ kind: "status" });
          try {
            await call({
              kind: "set_enabled",
              enabled,
              expected: before.configuration,
            });
          } catch (error) {
            // This passive barrier observes a possibly accepted selection. Never
            // replay a write whose acknowledgement may have been lost.
            await call({ kind: "status" });
            throw error;
          }
        },
      );
      const reconcile = app.fileEditingSessions.afterMutations(() =>
        app.configuration.reloadFromDisk({ materializeDefaults: false }),
      );
      const [selected, refreshed] = await Promise.allSettled([
        mutation,
        reconcile,
      ]);
      if (selected.status === "rejected") throw selected.reason;
      if (refreshed.status === "rejected") throw refreshed.reason;
    },
  };
}
