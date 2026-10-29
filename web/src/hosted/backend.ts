import type { NativeTasks } from "@lapis-notes/api/native-tasks";
import type { NativeEditingIntents } from "@lapis-notes/api/native-editing-intents";
import type { RepositoryServices } from "../history/services";
import type { NativeHistoryIndex } from "@lapis-notes/api/native-history-index";
import type { App, NativeBackgroundIndexing } from "@lapis-notes/api";
import type {
  NativeParticipantService,
  NativeQueueService,
  NativeTurnService,
} from "@lapis-notes/api/native-turns";
import type { AppDatabaseProvider } from "@lapis-notes/api/app-database";
import type { ServerVaultProfile } from "@lapis-notes/api/server-host";
import type { VaultAdapter } from "@lapis-notes/api/vault";
import type { AppPluginDistributionOptions } from "@lapis-notes/api";
import type { PluginAssetServer } from "@lapis-notes/api/plugin-assets";

export type ServerNativeChatLeave = "detach" | "close" | "disable";

export interface ServerNativeChatHost extends NativeTurnService {
  queue?: NativeQueueService & {
    selection(targetId: string): Promise<unknown>;
  };
  participants?: NativeParticipantService;
  allowed(): boolean;
  activate(): Promise<void>;
  leave(reason: ServerNativeChatLeave): Promise<void>;
  readText(path: string): Promise<string>;
  report?(error: Error): void;
}

export type HostedVault = {
  id: string;
  name: string;
  role: "owner" | "editor" | "viewer";
  archivedAt?: number;
};
export type HostedResources = {
  nativeTasks?: NativeTasks;
  nativeEditingIntents?: NativeEditingIntents;
  repositoryFactory?: (app: App) => RepositoryServices;
  nativeChat?: (app: App) => ServerNativeChatHost;
  backgroundIndexing?: NativeBackgroundIndexing;
  nativeHistoryIndex?: NativeHistoryIndex;
  adapter: VaultAdapter;
  database: AppDatabaseProvider;
  pluginAssetServer?: PluginAssetServer;
  pluginDistributionOptions?: AppPluginDistributionOptions;
  release(): Promise<void>;
};
export interface HostedBackend {
  readonly runtime: "node-server" | "rust-server";
  readonly protocolMajor: number;
  readonly capabilities: { manageVaults: boolean; transferVaults: boolean };
  info(): Promise<{ buildId: string; protocolMajor: number }>;
  current(): Promise<unknown>;
  login(username: string, password: string): Promise<void>;
  logout(): Promise<void>;
  vaults(): Promise<HostedVault[]>;
  create(name: string): Promise<{ vaultId: string }>;
  rename(vaultId: string, name: string): Promise<unknown>;
  archive(vaultId: string): Promise<unknown>;
  restore(vaultId: string): Promise<unknown>;
  importArchive(name: string, archive: Blob): Promise<void>;
  exportUrl(vaultId: string): string;
  prepare(profile: ServerVaultProfile): Promise<HostedResources>;
}
