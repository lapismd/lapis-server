import { retainHostedRead } from "../../connection/retained-read";
import {
  recoverBrowserAttachments,
  trackBrowserAttachment,
} from "./browser-attachment";
import type { OfflineWorkspace } from "../../offline/workspace";
import { isHostedTransportError } from "../../connection/connection";
import { createRustSkillManagement } from "./skill-management";
import { contentAccessPlugin } from "../../content-access/plugin";
import { hostedTerminalWorkspace } from "./terminal-registration";
import { createRustNativeEditingIntents } from "./native-editing-intents";
import { HistoryClient } from "@lapismd/jj-history";
import { createHostedRepository } from "../../history/services";
import { createHostedRestore } from "../../history/restore";
import { createHostedBackup } from "../../history/backup";
import { createHostedRemotes } from "../../history/remotes";
import { parseWorkingCopyStatus } from "@lapis-notes/history/repository/controllers/working-copy-status";
import type { HostedBackend } from "../backend";
import { RustHostError, RustHttpClient } from "./client";
import { RustDatabaseProvider } from "./database-provider";
import { RustVaultAdapter } from "./vault-adapter";
import { createRustPluginAssetServer } from "./plugin-assets";
import { preparePluginCredentials } from "./plugin-credentials";
import { createRustPluginDistributionFetch } from "./plugin-distribution";
import type { ManagedVault } from "../../../../crates/lapis-app-protocol/bindings/protocol";

export function createRustHostedBackend(
  host = new RustHttpClient(),
  {
    nativeMetadata = false,
    nativeHistory = false,
    nativeTasks = false,
    offline,
  }: {
    nativeMetadata?: boolean;
    nativeHistory?: boolean;
    nativeTasks?: boolean;
    offline?: OfflineWorkspace;
  } = {},
): HostedBackend {
  let account: string | undefined;
  const capabilities = { manageVaults: false, transferVaults: false };
  return {
    runtime: "rust-server",
    connection: host.connection,
    offline,
    protocolMajor: host.protocolMajor,
    capabilities,
    security: host.security,
    async info(signal) {
      const info = await host.info(signal);
      capabilities.manageVaults = info.manage_vaults;
      capabilities.transferVaults = info.transfer_vaults;
      return {
        buildId: info.build_id,
        protocolMajor: info.protocol_major,
        ...(info.passkeys ? { passkeys: true } : {}),
      };
    },
    async current(signal) {
      const session = await host.current(signal);
      account = session.username;
      return session;
    },
    login: (username, password) => host.login(username, password),
    logout: () => host.logout(),
    async vaults(signal) {
      return (await host.vaults(signal)).map((vault) => ({
        id: vault.vault_id,
        name: vault.name,
        role: "owner" as const,
        ...(vault.archived_at === undefined
          ? {}
          : { archivedAt: vault.archived_at }),
      }));
    },
    create: async (name) => {
      const vault = await host.json<ManagedVault>("/vaults", "POST", { name });
      return { vaultId: vault.vault_id };
    },
    rename: (id, name) =>
      host.json(`/vaults/${encodeURIComponent(id)}/name`, "POST", { name }),
    archive: (id) =>
      host.json(`/vaults/${encodeURIComponent(id)}/archive`, "POST"),
    restore: (id) =>
      host.json(`/vaults/${encodeURIComponent(id)}/restore`, "POST"),
    importArchive: async (name, archive) => {
      const response = await host.response(
        `/vaults/import?name=${encodeURIComponent(name)}`,
        {
          method: "POST",
          headers: { "content-type": "application/zip" },
          body: archive,
        },
      );
      await response.json();
    },
    exportUrl: (id) => host.url(`/vaults/${encodeURIComponent(id)}/export`),
    async prepare(profile) {
      const client = await host.attach(profile.id);
      const pluginSecretsHost = await preparePluginCredentials(client);
      const owner = account;
      if (owner && offline?.current.grant?.account === owner)
        await recoverBrowserAttachments(host, client, owner);
      const tracking =
        offline && owner
          ? await trackBrowserAttachment(client, owner, offline)
          : undefined;
      const adapter = new RustVaultAdapter(client, profile.name, offline);
      const historyIndexStatus = retainHostedRead(host.connection, () =>
        client.historyIndex({ kind: "status" }),
      );
      return {
        draftRemote: adapter.draftRemote,
        revalidate: (signal) => client.revalidate(signal),
        workspaceHost: {
          ...hostedTerminalWorkspace(client, [
            {
              plugin: contentAccessPlugin(host, profile.id),
              required: true,
              enabledByDefault: true,
            },
          ]),
          pluginSecretsHost,
        },
        adapter,
        skillManagement: createRustSkillManagement(client),
        nativeTasks: nativeTasks
          ? { call: (command) => client.tasks(command) }
          : undefined,
        nativeEditingIntents:
          nativeTasks || nativeHistory
            ? createRustNativeEditingIntents(client, tracking)
            : undefined,
        repositoryFactory: nativeHistory
          ? (app) =>
              createHostedRepository(
                new HistoryClient(profile.id, {
                  invoke: (command, frame) => client.history(command, frame),
                }),
                createHostedRestore((command) =>
                  client.repositoryRestore(command),
                ),
                createHostedBackup(app, (command) =>
                  client.repositoryBackup(command),
                ),
                {
                  discover: () =>
                    client.repositorySources({ kind: "discover" }),
                  open: async () => {
                    const source = await client.repositorySources({
                      kind: "open_original",
                    });
                    return new HistoryClient(source.token!, {
                      invoke: (command, frame) =>
                        client.history(command, frame),
                    });
                  },
                },
                {
                  remotes: createHostedRemotes(
                    (command) => client.repositoryRemotes(command),
                    client.signal,
                    undefined,
                    (command) => client.repositorySynchronization(command),
                    host.connection,
                  ),
                  workingCopy: async () =>
                    parseWorkingCopyStatus(
                      await client.repositoryWorkingCopy(),
                    ),
                },
              )
          : undefined,
        backgroundIndexing:
          nativeMetadata || nativeTasks
            ? { call: (command) => client.indexing(command) }
            : undefined,
        nativeHistoryIndex: nativeHistory
          ? {
              call: (command) =>
                command.kind === "status"
                  ? historyIndexStatus()
                  : client.historyIndex(command),
            }
          : undefined,
        nativeChat: (app) => ({
          allowed: () =>
            app.workspaceTrust.trusted &&
            !app.safeMode.disableOptionalCorePlugins,
          activate: async () => {},
          call: (command) => client.turns(command),
          participants: { call: (command) => client.participants(command) },
          queue: {
            call: (command) => client.queues(command),
            selection: async (targetId) => {
              const value = await client.turns({ kind: "targets" });
              return value.kind === "targets"
                ? value.targets.find((target) => target.id === targetId)
                    ?.queue_selection
                : undefined;
            },
          },
          readText: (path) => adapter.read(path),
          leave: async (reason) => {
            if (reason === "detach") return;
            try {
              await client.turns({ kind: "set_enabled", enabled: false });
            } catch (error) {
              // An unselected engine has no work to fence or drain.
              if (
                !(
                  error instanceof RustHostError &&
                  error.status === 501 &&
                  error.code === "UNSUPPORTED"
                )
              )
                throw error;
            }
          },
          report: (error) => {
            if (isHostedTransportError(error)) {
              host.connection.suspect();
              return;
            }
            app.notifications.notify({
              title: "AI",
              source: "ai",
              severity: "error",
              message: error.message,
            });
          },
        }),
        pluginAssetServer: createRustPluginAssetServer(client, adapter),
        database: new RustDatabaseProvider(client),
        pluginDistributionOptions: {
          fetch: createRustPluginDistributionFetch(client),
        },
        async release() {
          try {
            await client.close();
          } finally {
            tracking?.dispose();
          }
        },
      };
    },
  };
}
