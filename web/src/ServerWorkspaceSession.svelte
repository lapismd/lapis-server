<script lang="ts">
  import { onMount, untrack } from "svelte";
  import {
    App,
    provideApplicationState,
    type AppPluginDistributionOptions,
    type PluginAssetServer,
    type VaultSession,
  } from "@lapis-notes/api";
  import {
    appStatePath,
    DEFAULT_APP_STATE_DIRECTORY,
  } from "@lapis-notes/api/path";
  import type { ServerVaultProfile } from "@lapis-notes/api/server-host";
  import { FileExplorerPlugin } from "@lapis-notes/file-explorer";
  import { MarkdownPlugin } from "@lapis-notes/markdown";
  import markdownStyles from "@lapis-notes/markdown/styles.css?inline";
  import { SearchPlugin } from "@lapis-notes/search";
  import searchStyles from "@lapis-notes/search/styles.css?inline";
  import { SourceEditorPlugin } from "@lapis-notes/source-editor";
  import sourceEditorStyles from "@lapis-notes/source-editor/styles.css?inline";
  import { WorkspaceShell } from "@lapis-notes/workspace";
  import type { WorkspaceNavigation } from "@lapismd/design-core/workspace/app-shell";

  let {
    profile,
    session,
    onReady,
    onOpenRecent,
    onManageVaults,
    listProfiles,
    pluginAssetServer,
    pluginDistributionOptions,
  }: {
    profile: ServerVaultProfile;
    session: VaultSession;
    onReady(): void;
    onOpenRecent(profile: ServerVaultProfile): Promise<void>;
    onManageVaults(): Promise<void>;
    listProfiles: () => Promise<ServerVaultProfile[]>;
    pluginAssetServer?: PluginAssetServer;
    pluginDistributionOptions?: AppPluginDistributionOptions;
  } = $props();

  const app = untrack(
    () =>
      new App({
        version: "0.1.0",
        configPath: appStatePath("app.json"),
        session,
        pluginAssetServer,
        pluginDistributionOptions,
        markdownRenderer: async () => {},
      }),
  );
  provideApplicationState(app);

  let ready = $state(false);
  let failure = $state("");
  let disposed = false;
  let vaults = $state.raw<ServerVaultProfile[]>([]);

  const navigation = $derived<WorkspaceNavigation>({
    currentLabel: profile.name,
    menuLabel: "Vaults",
    manageLabel: "Manage vaults",
    emptyLabel: "No other vaults",
    items: vaults
      .filter((vault) => vault.id !== profile.id)
      .map((vault) => ({ id: vault.id, label: vault.name })),
    onSelect: async (item) => {
      const next = vaults.find((vault) => vault.id === item.id);
      if (next) await onOpenRecent(next);
    },
    onManage: () => onManageVaults(),
  });

  onMount(() => {
    void start();
  });

  async function start(): Promise<void> {
    if (disposed) return;
    try {
      app.plugins.registerStaticPlugins([
        {
          plugin: SourceEditorPlugin,
          required: false,
          enabledByDefault: true,
          styles: sourceEditorStyles,
        },
        {
          plugin: MarkdownPlugin,
          required: false,
          enabledByDefault: true,
          styles: markdownStyles,
        },
        {
          plugin: FileExplorerPlugin,
          required: false,
          enabledByDefault: true,
        },
        {
          plugin: SearchPlugin,
          required: false,
          enabledByDefault: true,
          styles: searchStyles,
        },
      ]);
      await app.vault.load();
      await app.configuration.load();
      await app.plugins.loadPlugins({
        communityPlugins: "configured",
        optionalCorePlugins: "configured",
      });
      await app.workspace.loadLayout();
      vaults = await listProfiles();
      if (disposed) return;
      ready = true;
      onReady();
    } catch (error) {
      failure = error instanceof Error ? error.message : String(error);
    }
  }

  async function writeLayout(): Promise<void> {
    if (!app.workspace.layoutReady) return;
    await session.vaultAdapter.mkdir(DEFAULT_APP_STATE_DIRECTORY, {
      recursive: true,
    });
    await session.vaultAdapter.write(
      appStatePath("workspace.json"),
      JSON.stringify(app.workspace.getLayout(), null, 2),
    );
  }

  export async function persistLayout(): Promise<void> {
    if (disposed) return;
    await writeLayout();
  }

  export function canRecoverLeave(): boolean {
    return !disposed;
  }

  export async function dispose(persist: boolean): Promise<void> {
    if (disposed) return;
    if (persist) await writeLayout();
    disposed = true;
    ready = false;
    await app.workspace.disposeWorkspaceHost();
    await session.close();
  }
</script>

<section class="server-workspace-session" data-vault-id={profile.id}>
  {#if failure}
    <p role="alert">{failure}</p>
  {:else if ready}
    <WorkspaceShell
      {app}
      displayMode="desktop"
      workspaceLabel={profile.name}
      workspaceNavigation={navigation}
    />
  {/if}
</section>
