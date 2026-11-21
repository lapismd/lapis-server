import type { App } from "@lapis-notes/api";
import type { HostedConnection } from "../connection/connection";
import {
  isOfflineMarkdown,
  type DraftRemote,
  type OfflineWorkspace,
} from "./workspace";

export function trackHostedNotes(
  app: App,
  scope: { account: string; vault: string },
  offline: OfflineWorkspace,
  connection: HostedConnection,
  remote: DraftRemote,
) {
  let alive = true;
  let generation = 0;
  const refresh = async () => {
    const revision = ++generation;
    const path = app.workspace.getActiveFile()?.path;
    if (offline.current.grant?.account !== scope.account) return;
    await offline.select(scope.account, scope.vault, path);
    if (connection.phase !== "connected" || !path || !isOfflineMarkdown(path))
      return;
    const snapshot = await remote.read(path);
    if (alive && snapshot && revision === generation)
      await offline.cache(scope.account, scope.vault, path, snapshot);
  };
  const changed = () => {
    void refresh().catch(() => {});
  };
  const file = app.workspace.on("file-open", changed);
  const connected = connection.subscribe(({ phase }) => {
    if (phase === "connected") changed();
  });
  changed();
  return {
    refresh,
    close() {
      alive = false;
      connected();
      app.workspace.offref(file);
    },
  };
}
