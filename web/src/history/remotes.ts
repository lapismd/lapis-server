import type { HostedConnection } from "../connection/connection";
import { retainHostedRead } from "../connection/retained-read";
import type { RepositoryRemotesService } from "@lapis-notes/history/repository";
import type {
  RepositoryRemotesCommand,
  RepositoryRemotesSnapshot,
  RepositorySyncCommand,
  RepositorySyncSnapshot,
} from "../../../crates/lapis-app-protocol/bindings/protocol";

type Handoff = {
  copy(code: string): Promise<void>;
  open(url: string): void;
};

/** The native owner admits endpoints, credentials and publication identities. */
export function createHostedRemotes(
  call: (
    command?: RepositoryRemotesCommand,
  ) => Promise<RepositoryRemotesSnapshot>,
  lifetime: AbortSignal,
  handoff: Handoff = {
    copy: (code) => navigator.clipboard.writeText(code),
    open: (url) => {
      window.open(url, "_blank", "noopener,noreferrer");
    },
  },
  synchronize?: (
    command?: RepositorySyncCommand,
  ) => Promise<RepositorySyncSnapshot>,
  connection?: HostedConnection,
): RepositoryRemotesService & { close(): void } {
  let closed = false;
  const requireOpen = () => {
    if (closed || lifetime.aborted)
      throw new Error("Repository remotes are closed");
  };
  const request = async (command?: RepositoryRemotesCommand) => {
    requireOpen();
    const snapshot = await call(command);
    requireOpen();
    return snapshot;
  };
  const readSynchronization =
    synchronize && retainHostedRead(connection, () => synchronize());
  return {
    synchronization:
      synchronize && readSynchronization
        ? {
            read: async () => {
              requireOpen();
              const snapshot = await readSynchronization();
              requireOpen();
              return snapshot;
            },
            execute: async (command) => {
              requireOpen();
              const snapshot = await synchronize(command);
              requireOpen();
              return snapshot;
            },
          }
        : undefined,
    read: () => request(),
    execute: (command) => request(command),
    async openSignIn() {
      const snapshot = await request();
      const authorization = snapshot.settings.authorization;
      if (
        !snapshot.capabilities.githubDeviceSignIn ||
        !snapshot.settings.deviceSignInAvailable ||
        authorization?.state !== "pending" ||
        !authorization.code.trim()
      )
        throw new Error("GitHub authorization is unavailable. Sign in again.");
      let url: URL;
      try {
        url = new URL(authorization.url);
      } catch {
        throw new Error("GitHub verification page is unavailable");
      }
      if (
        url.protocol !== "https:" ||
        url.username ||
        url.password ||
        url.search ||
        url.hash ||
        url.pathname !== "/login/device"
      )
        throw new Error("GitHub verification page is unavailable");
      // Enterprise origins are admitted by the native account manager. No code
      // or token is added to the URL, browser storage or application data.
      await handoff.copy(authorization.code);
      requireOpen();
      handoff.open(url.href);
    },
    close() {
      closed = true;
    },
  };
}
