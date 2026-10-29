import { createRustNativeEditingIntents } from "./native-editing-intents";
import { HistoryClient } from "@lapismd/jj-history";
import { createHostedRepository } from "../../history/services";
import { createHostedRestore } from "../../history/restore";
import { createHostedBackup } from "../../history/backup";
import type { HostedBackend } from "../backend";
import { RustHttpClient } from "./client";
import { RustDatabaseProvider } from "./database-provider";
import { RustVaultAdapter } from "./vault-adapter";
import { createRustPluginAssetServer } from "./plugin-assets";
import { createRustPluginDistributionFetch } from "./plugin-distribution";
import type { ManagedVault } from "../../../../crates/lapis-app-protocol/bindings/protocol";

export function createRustHostedBackend(
  host = new RustHttpClient(),
  {
    nativeMetadata = false,
    nativeHistory = false,
    nativeTasks = false,
  }: {
    nativeMetadata?: boolean;
    nativeHistory?: boolean;
    nativeTasks?: boolean;
  } = {},
): HostedBackend {
  const capabilities = { manageVaults: false, transferVaults: false };
  return {
    runtime: "rust-server",
    protocolMajor: host.protocolMajor,
    capabilities,
    async info() {
      const info = await host.info();
      capabilities.manageVaults = info.manage_vaults;
      capabilities.transferVaults = info.transfer_vaults;
      return { buildId: info.build_id, protocolMajor: info.protocol_major };
    },
    current: () => host.current(),
    login: (username, password) => host.login(username, password),
    logout: () => host.logout(),
    async vaults() {
      return (await host.vaults()).map((vault) => ({
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
      const adapter = new RustVaultAdapter(client, profile.name);
      return {
        adapter,
        nativeTasks: nativeTasks
          ? { call: (command) => client.tasks(command) }
          : undefined,
        nativeEditingIntents: nativeTasks
          ? createRustNativeEditingIntents(client)
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
              )
          : undefined,
        backgroundIndexing:
          nativeMetadata || nativeTasks
            ? { call: (command) => client.indexing(command) }
            : undefined,
        nativeHistoryIndex: nativeHistory
          ? { call: (command) => client.historyIndex(command) }
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
            await client.turns({ kind: "set_enabled", enabled: false });
          },
          report: (error) =>
            app.notifications.notify({
              title: "AI",
              source: "ai",
              severity: "error",
              message: error.message,
            }),
        }),
        pluginAssetServer: createRustPluginAssetServer(client, adapter),
        database: new RustDatabaseProvider(client),
        pluginDistributionOptions: {
          fetch: createRustPluginDistributionFetch(client),
        },
        release: () => client.close(),
      };
    },
  };
}
