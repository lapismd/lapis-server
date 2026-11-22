<script lang="ts">
  import McpConsent from "./account/McpConsent.svelte";
  import AccountSecurity from "./account/AccountSecurity.svelte";
  import { supportsPasskeys, securityError } from "./account/passkeys";
  import serverLogo from "./assets/lapis-logo.svg?url";
  import Archive from "@lucide/svelte/icons/archive";
  import Download from "@lucide/svelte/icons/download";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import FolderInput from "@lucide/svelte/icons/folder-input";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import KeyRound from "@lucide/svelte/icons/key-round";
  import LogIn from "@lucide/svelte/icons/log-in";
  import LogOut from "@lucide/svelte/icons/log-out";
  import PenLine from "@lucide/svelte/icons/pen-line";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Search from "@lucide/svelte/icons/search";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import X from "@lucide/svelte/icons/x";
  import {
    createVaultSession,
    pickFileSystemAccessDirectoryHandle,
    type BrowserFileSystemDirectoryHandle,
    type VaultSession,
  } from "@lapis-notes/api";
  import { type ServerVaultProfile } from "@lapis-notes/api/server-host";
  import type {
    HostedBackend,
    HostedResources,
    HostedVault,
  } from "./hosted/backend";
  import * as Button from "@lapismd/design-core/shadcn/button";
  import * as Card from "@lapismd/design-core/shadcn/card";
  import * as CommandView from "@lapismd/design-core/shadcn/command-view";
  import * as Dialog from "@lapismd/design-core/shadcn/dialog";
  import * as DropdownMenu from "@lapismd/design-core/shadcn/dropdown-menu";
  import * as Input from "@lapismd/design-core/shadcn/input";
  import { onMount, tick, untrack, type Snippet } from "svelte";
  import {
    archiveServerImportDirectory,
    canPickServerImportDirectory,
  } from "./server-vault-import";
  import {
    NotesWorkspaceSession,
    type NotesWorkspaceSessionHandle,
    type NotesWorkspaceHost,
  } from "@lapis-notes/app-profile/workspace-session";
  import {
    createVaultRestoration,
    type VaultRestoration,
  } from "./vault-restoration";

  import ConnectionStatus from "./connection/ConnectionStatus.svelte";
  import {
    HostedConnection,
    isHostedTransportError,
  } from "./connection/connection";
  import { registerConnectionStatus } from "./connection/status-contribution";
  import { RustHostError } from "./hosted/rust/client";

  import type { App } from "@lapis-notes/api";
  import OfflineVault from "./offline/OfflineVault.svelte";
  import { OfflineWorkspace } from "./offline/workspace";
  import { trackHostedNotes } from "./offline/observe-editor";
  import { flushOfflineWriters } from "./offline/persistence";
  import type { ServerPwa } from "./pwa/registration";
  import { readable } from "svelte/store";

  type VaultRow = HostedVault;
  type Prepared = HostedResources & {
    profile: ServerVaultProfile;
    session: VaultSession;
    host: NotesWorkspaceHost;
    token: symbol;
    ready: boolean;
    onReady(app: App): void;
  };

  let {
    backend,
    restoration = createVaultRestoration(),
    consentId,
    accountSecurityPage,
    pwa,
  }: {
    backend: HostedBackend;
    restoration?: VaultRestoration;
    consentId?: string;
    accountSecurityPage?: Snippet;
    pwa?: ServerPwa;
  } = $props();
  const connection = untrack(
    () => backend.connection ?? new HostedConnection(),
  );
  const offline = untrack(() => backend.offline ?? new OfflineWorkspace());
  const emptyPwa = readable({ canInstall: false, updateAvailable: false });
  const pwaState = $derived(pwa ?? emptyPwa);
  let showCached = $state(false);
  let activeApp: App | undefined;
  let observer: ReturnType<typeof trackHostedNotes> | undefined;
  let sessionExpiry: number | undefined;

  type HostState =
    | "loading"
    | "connection"
    | "login"
    | "vaults"
    | "opening"
    | "workspace"
    | "protocol";
  let hostState: HostState = $state("loading");
  let username = $state("");
  let authenticatedUsername = $state<string | null>(null);
  let disposed = false;
  let password = $state("");
  let passkeysEnabled = $state(false);
  let passkeysAvailable = $state(false);
  const hasAccountSecurity = $derived(
    Boolean(backend.security || accountSecurityPage),
  );
  let accountOpen = $state(false);
  const showAccountSecurityPage = $derived(accountOpen && hasAccountSecurity);

  let passkeyBusy = $state(false);
  let newVaultName = $state("");
  let errorMessage = $state("");
  let actionMessage = $state("");
  let buildId = $state("");
  let vaults: VaultRow[] = $state.raw([]);
  let prepared: Prepared | null = $state(null);
  let sessionComponent: NotesWorkspaceSessionHandle | null = $state(null);
  let openingFlight: Promise<void> | undefined;
  let importInput: HTMLInputElement | null = $state(null);
  let activeAction = $state<string | null>(null);
  let createDialogOpen = $state(false);
  let renameDialogOpen = $state(false);
  let renameTarget = $state.raw<VaultRow | null>(null);
  let renameVaultName = $state("");
  let directoryDialogOpen = $state(false);
  let directoryHandle = $state.raw<BrowserFileSystemDirectoryHandle | null>(
    null,
  );
  let directoryVaultName = $state("");
  let importProgress = $state("");
  let vaultQuery = $state("");
  let vaultDialogOpen = $state(false);

  const canImportDirectory = canPickServerImportDirectory();
  const searchableVaults = $derived(
    vaults.filter((vault) => vault.archivedAt === undefined),
  );
  const filteredVaults = $derived(
    searchableVaults.filter((vault) => {
      const query = vaultQuery.trim().toLowerCase();
      return (
        !query ||
        vault.name.toLowerCase().includes(query) ||
        vault.id.toLowerCase().includes(query) ||
        vault.role.toLowerCase().includes(query)
      );
    }),
  );

  $effect(() => {
    if (showCached && !$offline.grant) showCached = false;
  });

  onMount(() => {
    const stopConnection = connection.start(probeConnection);
    const stopOffline = backend.offline?.start();
    void initialize();
    return () => {
      stopConnection();
      stopOffline?.();
      void dispose();
    };
  });

  function profileFor(row: VaultRow): ServerVaultProfile {
    const now = Date.now();
    return {
      id: row.id,
      name: row.name,
      kind: "server-hosted",
      role: row.role,
      archived: row.archivedAt !== undefined,
      createdAt: now,
      updatedAt: now,
    };
  }

  async function initialize(): Promise<void> {
    errorMessage = "";
    try {
      const signal = AbortSignal.timeout(5000);
      const info = await backend.info(signal);
      if (disposed) return;
      buildId = info.buildId;
      passkeysEnabled = info.passkeys === true;
      passkeysAvailable = passkeysEnabled && supportsPasskeys();
      if (info.protocolMajor !== backend.protocolMajor) {
        hostState = "protocol";
        return;
      }
      try {
        await enterAuthenticatedHost(signal);
      } catch (error) {
        if (isHostedTransportError(error) || signal.aborted) throw error;
        connection.reached("sign-in");
        hostState = "login";
      }
    } catch (error) {
      if (disposed) return;
      if (
        isHostedTransportError(error) ||
        (error instanceof Error && error.name === "TimeoutError")
      ) {
        connection.lost();
        hostState = "connection";
        if (backend.offline && !consentId) {
          await offline.refresh().catch(() => {});
          if (!disposed && hostState === "connection")
            showCached = !!offline.current.grant?.selection;
        }
      } else {
        errorMessage = error instanceof Error ? error.message : String(error);
        hostState = "login";
      }
    }
  }

  async function probeConnection(signal: AbortSignal) {
    if (hostState === "loading") throw Error("Initial connection is pending");
    const info = await backend.info(signal);
    signal.throwIfAborted();
    if (info.protocolMajor !== backend.protocolMajor) {
      buildId = info.buildId;
      hostState = "protocol";
      return "update-required";
    }
    try {
      if (hostState === "connection") {
        await enterAuthenticatedHost(signal);
      } else {
        const identity = await backend.current(signal);
        if (authenticatedUsername !== identity.username) {
          offline.restrict();
          hostState = "login";
          return "sign-in";
        }
        const latest = await backend.vaults(signal);
        signal.throwIfAborted();
        vaults = latest;
        await authorizeOffline(identity);
        if (prepared) {
          const current = latest.find(
            (vault) =>
              vault.id === prepared?.profile.id &&
              vault.archivedAt === undefined,
          );
          if (!current) {
            hostState = "vaults";
            showCached = false;
          } else if (
            prepared.revalidate &&
            !(await prepared.revalidate(signal))
          ) {
            showCached = false;
            await openVault(
              current,
              activeApp?.workspace.getActiveFile()?.path,
            );
          }
        }
      }
      return "connected";
    } catch (error) {
      if (!(error instanceof RustHostError) || error.status !== 401)
        throw error;
      offline.restrict();
      showCached = false;
      hostState = prepared ? "connection" : "login";
      return "sign-in";
    }
  }

  function requestSignIn() {
    showCached = false;
    hostState = "login";
  }

  async function authorizeOffline(identity: {
    username: string;
    expires_at?: number;
  }) {
    sessionExpiry = identity.expires_at;
    if (backend.offline)
      await offline
        .authorize(identity.username, identity.expires_at, searchableVaults)
        .catch(() => {});
  }

  async function enableOffline() {
    if (!authenticatedUsername || !sessionExpiry || !prepared?.ready)
      throw Error("Open an authenticated vault before enabling offline data.");
    await offline.enable(
      authenticatedUsername,
      sessionExpiry,
      searchableVaults,
      prepared.profile.id,
    );
    await observer?.refresh();
  }

  async function resumeWorkspace(path?: string) {
    await flushOfflineWriters();
    showCached = false;
    if (
      path &&
      activeApp &&
      connection.phase === "connected" &&
      activeApp.workspace.getActiveFile()?.path !== path
    ) {
      await activeApp.workspace.openLinkText(path, path);
    }
  }

  async function applyPwaUpdate() {
    await pwa?.applyUpdate(async () => {
      await flushOfflineWriters();
      await disposeWorkspace(true);
    });
  }

  async function login(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    errorMessage = "";
    try {
      await backend.login(username, password);
      password = "";
      await enterAuthenticatedHost();
    } catch (error) {
      password = "";
      errorMessage = error instanceof Error ? error.message : String(error);
    }
  }

  async function signInWithPasskey() {
    if (passkeyBusy || !backend.security) return;
    passkeyBusy = true;
    errorMessage = "";
    try {
      await backend.security.signIn(username);
      password = "";
      await enterAuthenticatedHost();
    } catch (error) {
      errorMessage = securityError(error);
    } finally {
      passkeyBusy = false;
    }
  }

  async function refreshVaults(): Promise<void> {
    vaults = await backend.vaults();
  }

  async function enterAuthenticatedHost(signal?: AbortSignal): Promise<void> {
    const identity = await backend.current(signal);
    signal?.throwIfAborted();
    if (disposed) return;
    authenticatedUsername = identity.username;
    vaults = await backend.vaults(signal);
    signal?.throwIfAborted();
    if (disposed) return;
    await authorizeOffline(identity);
    if (disposed) return;
    connection.reached("connected");
    hostState = "vaults";
    if (consentId) return;
    const cached = showCached ? offline.current.grant?.selection : undefined;
    const vaultId = cached?.vault ?? restoration.take(identity.username);
    if (!vaultId) return;
    const vault = vaults.find(
      (row) => row.id === vaultId && row.archivedAt === undefined,
    );
    if (!vault) {
      errorMessage =
        "Your previously selected vault is unavailable. Choose a vault.";
      return;
    }
    await openVault(vault, cached?.path);
  }

  function activeProfiles() {
    return vaults
      .filter((vault) => vault.archivedAt === undefined)
      .map(profileFor);
  }

  async function createVault(): Promise<void> {
    const name = newVaultName.trim();
    if (!name) return;
    errorMessage = "";
    actionMessage = "";
    activeAction = "create";
    try {
      const created = await backend.create(name);
      newVaultName = "";
      createDialogOpen = false;
      await refreshVaults();
      const row = vaults.find((vault) => vault.id === created.vaultId);
      if (row) await openVault(row);
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
    } finally {
      activeAction = null;
    }
  }

  function openVault(row: VaultRow, path?: string): Promise<void> {
    const flight = (openingFlight ?? Promise.resolve()).then(() => {
      if (!disposed) return openSelectedVault(row, path);
    });
    openingFlight = flight;
    const finish = () => {
      if (openingFlight === flight) openingFlight = undefined;
    };
    void flight.then(finish, finish);
    return flight;
  }

  async function openSelectedVault(
    row: VaultRow,
    path?: string,
  ): Promise<void> {
    if (row.archivedAt !== undefined) return;
    const reviewDrafts =
      showCached && offline.current.notes.some((entry) => entry.draft);
    hostState = "opening";
    errorMessage = "";
    try {
      await disposeWorkspace(true);
      const profile = profileFor(row);
      const resources = await backend.prepare(profile);
      try {
        const session = await createVaultSession(resources.adapter, {
          runtime: backend.runtime,
          profile,
          appDatabaseProvider: resources.database,
        });
        session.skillManagement = resources.skillManagement;
        session.nativeTasks = resources.nativeTasks;
        session.nativeEditingIntents = resources.nativeEditingIntents;
        session.backgroundIndexing = resources.backgroundIndexing;
        session.nativeHistoryIndex = resources.nativeHistoryIndex;
        const token = Symbol("vault workspace");
        const owner = authenticatedUsername;
        prepared = {
          ...resources,
          profile,
          session,
          token,
          ready: false,
          onReady: (app) => {
            if (
              disposed ||
              prepared?.token !== token ||
              hostState !== "opening"
            )
              return;
            prepared.ready = true;
            activeApp = app;
            if (owner) {
              restoration.remember(owner, profile.id);
              if (backend.offline && resources.draftRemote) {
                observer = trackHostedNotes(
                  app,
                  { account: owner, vault: profile.id },
                  offline,
                  connection,
                  resources.draftRemote,
                );
              }
            }
            if (path && app.workspace.getActiveFile()?.path !== path)
              void app.workspace.openLinkText(path, path).catch(() => {});
            hostState = "workspace";
            if (!reviewDrafts) showCached = false;
          },
          host: {
            ...resources.workspaceHost,
            attach: (app, syncHost) => {
              const releaseHost = resources.workspaceHost?.attach?.(
                app,
                syncHost,
              );
              const releaseStatus = registerConnectionStatus(app, {
                connection,
                signIn: requestSignIn,
                ...(backend.offline
                  ? {
                      offline,
                      enableOffline,
                      browseOffline: () => {
                        showCached = true;
                      },
                    }
                  : {}),
                pwa,
                updatePwa: applyPwaUpdate,
              });
              return () => {
                releaseStatus();
                releaseHost?.();
              };
            },
            dispose: async (app) => {
              try {
                await resources.workspaceHost?.dispose?.(app);
              } finally {
                await resources.release();
              }
            },
          },
        };
      } catch (error) {
        await resources.release();
        throw error;
      }
      await tick();
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
      hostState = sessionComponent?.canRecoverLeave() ? "workspace" : "vaults";
    }
  }

  async function disposeWorkspace(persistLayout: boolean): Promise<void> {
    const active = prepared;
    if (sessionComponent) {
      try {
        await sessionComponent.dispose(persistLayout);
      } catch (error) {
        if (sessionComponent.canRecoverLeave()) hostState = "workspace";
        throw error;
      }
    } else if (active) {
      await active.session.close();
      await active.release();
    }
    observer?.close();
    observer = undefined;
    activeApp = undefined;
    sessionComponent = null;
    prepared = null;
  }

  async function manageVaults(): Promise<void> {
    await refreshVaults();
    if (authenticatedUsername) restoration.forget(authenticatedUsername);
    hostState = "vaults";
  }

  function returnToWorkspace(): void {
    if (!prepared) return;
    accountOpen = false;
    errorMessage = "";
    if (prepared.ready && authenticatedUsername)
      restoration.remember(authenticatedUsername, prepared.profile.id);
    hostState = prepared.ready ? "workspace" : "opening";
  }

  function showVaultSearch(): void {
    vaultQuery = "";
    vaultDialogOpen = true;
  }

  async function openVaultFromSearch(row: VaultRow): Promise<void> {
    vaultDialogOpen = false;
    await openVault(row);
  }

  async function runVaultAction(
    actionId: string,
    action: () => Promise<void>,
    successMessage = "",
  ): Promise<void> {
    activeAction = actionId;
    errorMessage = "";
    actionMessage = "";
    try {
      await action();
      await refreshVaults();
      actionMessage = successMessage;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
    } finally {
      activeAction = null;
    }
  }

  async function archive(row: VaultRow): Promise<void> {
    await runVaultAction(
      `archive:${row.id}`,
      async () => {
        if (prepared?.profile.id === row.id) await disposeWorkspace(true);
        await backend.archive(row.id);
      },
      `${row.name} archived`,
    );
  }

  async function restore(row: VaultRow): Promise<void> {
    await runVaultAction(
      `restore:${row.id}`,
      () => backend.restore(row.id).then(() => undefined),
      `${row.name} restored`,
    );
  }

  function openRenameDialog(row: VaultRow): void {
    renameTarget = row;
    renameVaultName = row.name;
    renameDialogOpen = true;
  }

  async function renameVault(): Promise<void> {
    const target = renameTarget;
    const name = renameVaultName.trim();
    if (!target || !name || name === target.name) return;
    await runVaultAction(
      `rename:${target.id}`,
      () => backend.rename(target.id, name).then(() => undefined),
      `${name} renamed`,
    );
    if (!errorMessage) renameDialogOpen = false;
  }

  async function uploadVaultArchive(
    name: string,
    archive: Blob,
  ): Promise<void> {
    await backend.importArchive(name, archive);
  }

  async function importZip(): Promise<void> {
    const file = importInput?.files?.[0];
    if (!file) return;
    const name = file.name.replace(/\.zip$/iu, "") || "Imported vault";
    await runVaultAction(
      "import:zip",
      () => uploadVaultArchive(name, file),
      `${name} imported`,
    );
    if (importInput) importInput.value = "";
  }

  function isAbortError(error: unknown): boolean {
    return (error as { name?: string } | undefined)?.name === "AbortError";
  }

  async function pickImportDirectory(): Promise<void> {
    errorMessage = "";
    actionMessage = "";
    try {
      const handle = await pickFileSystemAccessDirectoryHandle({
        id: "lapis-server-import-vault",
        mode: "read",
      });
      directoryHandle = handle;
      directoryVaultName = handle.name || "Imported vault";
      directoryDialogOpen = true;
    } catch (error) {
      if (isAbortError(error)) return;
      errorMessage = error instanceof Error ? error.message : String(error);
    }
  }

  async function importDirectory(): Promise<void> {
    const handle = directoryHandle;
    const name = directoryVaultName.trim();
    if (!handle || !name) return;
    importProgress = `Scanning ${handle.name}…`;
    await runVaultAction(
      "import:directory",
      async () => {
        const archive = await archiveServerImportDirectory(handle, {
          onProgress: ({ collectedFiles, currentPath }) => {
            importProgress = `Preparing ${collectedFiles} file${collectedFiles === 1 ? "" : "s"}: ${currentPath ?? handle.name}`;
          },
        });
        importProgress = `Uploading ${archive.fileCount} file${archive.fileCount === 1 ? "" : "s"}…`;
        const buffer = archive.bytes.slice().buffer as ArrayBuffer;
        await uploadVaultArchive(
          name,
          new Blob([buffer], { type: "application/zip" }),
        );
      },
      `${name} imported`,
    );
    importProgress = "";
    if (!errorMessage) {
      directoryDialogOpen = false;
      directoryHandle = null;
      directoryVaultName = "";
    }
  }

  function downloadVault(row: VaultRow): void {
    const link = document.createElement("a");
    link.href = backend.exportUrl(row.id);
    link.download = "";
    document.body.append(link);
    link.click();
    link.remove();
  }

  async function logout(): Promise<void> {
    errorMessage = "";
    try {
      await disposeWorkspace(true);
      await backend.logout();
      showCached = false;
      if (backend.offline) void offline.clear().catch(() => {});
      accountOpen = false;
      if (authenticatedUsername) restoration.forget(authenticatedUsername);
      authenticatedUsername = null;
      vaults = [];
      username = "";
      hostState = "login";
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
      hostState = sessionComponent?.canRecoverLeave() ? "workspace" : "vaults";
    }
  }

  /** Hosts and catalog fixtures await owned teardown before unmounting. */
  export async function dispose(): Promise<void> {
    disposed = true;
    try {
      await openingFlight;
    } finally {
      await disposeWorkspace(false);
    }
  }
</script>

<div
  class="server-host"
  data-server-host-state={hostState}
  role={hostState === "workspace" || showCached ? undefined : "main"}
>
  {#if prepared}
    <div
      class="server-workspace"
      hidden={showCached ||
        (hostState !== "workspace" && hostState !== "opening")}
      inert={showCached ||
        (hostState !== "workspace" && hostState !== "opening")}
    >
      <NotesWorkspaceSession
        bind:this={
          () => sessionComponent,
          (component) => {
            // Owned teardown can outlive Svelte's binding during unmount.
            if (component) sessionComponent = component;
          }
        }
        runtime={backend.runtime}
        profile={prepared.profile}
        session={prepared.session}
        host={prepared.host}
        listProfiles={async () => activeProfiles()}
        nativeChat={prepared.nativeChat}
        repositoryFactory={prepared.repositoryFactory}
        pluginAssetServer={prepared.pluginAssetServer}
        pluginDistributionOptions={prepared.pluginDistributionOptions}
        onReady={prepared.onReady}
        onOpenRecent={async (profile) => {
          const row = vaults.find((vault) => vault.id === profile.id);
          if (row) await openVault(row);
        }}
        onManageVaults={manageVaults}
      />
    </div>
  {/if}

  {#if showCached && $offline.grant}
    <OfflineVault
      {offline}
      {connection}
      signIn={requestSignIn}
      remote={$connection.phase === "connected"
        ? prepared?.draftRemote
        : undefined}
      resume={prepared?.ready
        ? (path) => void resumeWorkspace(path).catch(() => {})
        : undefined}
    />
  {:else if hostState === "loading" || (hostState === "opening" && !prepared)}
    <section class="server-centered-surface" aria-live="polite">
      <Card.Root class="server-status-card">
        <Card.Header>
          <p class="server-eyebrow">Lapis Server</p>
          <h1 class="server-status-title">
            {hostState === "opening" ? "Opening vault" : "Connecting"}
          </h1>
          <Card.Description>
            Checking the server and preparing your managed workspace…
          </Card.Description>
        </Card.Header>
      </Card.Root>
    </section>
  {:else if hostState === "connection"}
    <section class="server-centered-surface" aria-label="Lapis Server">
      <Card.Root class="server-status-card">
        <Card.Header>
          <h1 class="server-status-title">Lapis Server</h1>
          <Card.Description>Waiting for the server connection.</Card.Description
          >
        </Card.Header>
      </Card.Root>
    </section>
    <footer class="server-connection-footer" aria-label="Status bar">
      <ConnectionStatus {connection} signIn={requestSignIn} {offline} />
    </footer>
  {:else if hostState === "protocol"}
    <section class="server-centered-surface" role="alert">
      <Card.Root class="server-status-card">
        <Card.Header>
          <p class="server-eyebrow">Update required</p>
          <h1 class="server-status-title">This Lapis tab is out of date</h1>
          <Card.Description>
            The server protocol changed. Reload to use the client shipped with
            build {buildId}.
          </Card.Description>
        </Card.Header>
        <Card.Footer>
          <Button.Root
            class="server-full-button"
            onclick={() => location.reload()}
          >
            Reload Lapis
          </Button.Root>
        </Card.Footer>
      </Card.Root>
    </section>
  {:else if hostState === "login"}
    <section class="server-centered-surface server-login" data-server-login>
      <form onsubmit={login}>
        <Card.Root class="server-login-card">
          <Card.Header class="server-login-header">
            <img
              src={serverLogo}
              alt=""
              class="server-login-logo"
              data-server-login-logo
            />
            <div class="server-login-heading">
              <p class="server-eyebrow">Lapis Notes</p>
              <h1 class="server-login-title">Sign in</h1>
              <Card.Description>
                Open your managed Lapis vaults from this server.
              </Card.Description>
            </div>
          </Card.Header>
          <Card.Content class="server-login-fields">
            <label class="server-field">
              <span>Username</span>
              <Input.Root
                autocomplete="username"
                bind:value={username}
                required
              />
            </label>
            <label class="server-field">
              <span>Password</span>
              <Input.Root
                type="password"
                autocomplete="current-password"
                bind:value={password}
                required
              />
            </label>
            {#if errorMessage}
              <p class="server-error" role="alert">{errorMessage}</p>
            {/if}
            <Button.Root
              class="server-full-button"
              type="submit"
              data-server-login-submit
            >
              <LogIn aria-hidden="true" data-icon="inline-start" />
              Sign in
            </Button.Root>
            {#if passkeysAvailable}
              <Button.Root
                type="button"
                variant="outline"
                disabled={passkeyBusy || !username.trim()}
                onclick={() => void signInWithPasskey()}
              >
                <KeyRound aria-hidden="true" data-icon="inline-start" />
                Sign in with a passkey
              </Button.Root>
            {/if}
          </Card.Content>
        </Card.Root>
      </form>
    </section>
  {:else if hostState === "vaults" && consentId}
    <McpConsent
      requestId={consentId}
      username={authenticatedUsername}
      {vaults}
    />
  {:else if hostState === "vaults"}
    <section
      class="web-vault-launcher server-vault-launcher"
      data-server-vault-launcher
      aria-busy={activeAction !== null}
    >
      <div class="web-vault-launcher__content">
        <header class="server-launcher-header">
          <div class="server-brand-heading">
            <img src={serverLogo} alt="" class="server-launcher-logo" />
            <div class="server-brand-copy">
              <p class="server-eyebrow">Lapis Notes</p>
              <div class:launcher-copy-views={hasAccountSecurity}>
                <div inert={showAccountSecurityPage}>
                  <h1 class="server-launcher-title">Your vaults</h1>
                  <p class="server-launcher-description">
                    Open a managed server vault or bring an existing project
                    online.
                  </p>
                </div>
                {#if hasAccountSecurity}
                  <div inert={!showAccountSecurityPage}>
                    <h1
                      id="server-account-security-heading"
                      class="server-launcher-title"
                    >
                      Account &amp; security
                    </h1>
                    <p class="server-launcher-description">
                      Manage how you sign in to your server account.
                    </p>
                  </div>
                {/if}
              </div>
            </div>
          </div>
          <div class="server-launcher-actions">
            {#if hasAccountSecurity}
              <Button.Root
                variant="ghost"
                size="sm"
                class="account-security-switch"
                onclick={() => {
                  accountOpen = !showAccountSecurityPage;
                }}
              >
                {#if showAccountSecurityPage}
                  <!--
                    Lucide database-cog, https://lucide.dev/icons/database-cog
                    Copyright (c) 2026 Lucide Icons and Contributors
                    Permission to use, copy, modify, and/or distribute this software for any
                    purpose with or without fee is hereby granted, provided that the above
                    copyright notice and this permission notice appear in all copies.
                    THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
                    WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
                    MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
                    ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
                    WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
                    ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
                    OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
                  -->
                  <svg
                    class="lucide-database-cog"
                    aria-hidden="true"
                    data-icon="inline-start"
                    width="24"
                    height="24"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  >
                    <path d="m14.305 19.53.923-.382" />
                    <path d="m15.228 16.852-.923-.383" />
                    <path d="m16.852 15.228-.383-.923" />
                    <path d="m16.852 20.772-.383.924" />
                    <path d="m19.148 15.228.383-.923" />
                    <path d="m19.53 21.696-.382-.924" />
                    <path d="m20.772 16.852.924-.383" />
                    <path d="m20.772 19.148.924.383" />
                    <path d="M21 10.5V5" />
                    <path d="M3 12a9 3 0 0 0 7.599 2.963" />
                    <path d="M3 5v14a9 3 0 0 0 8.148 2.987" />
                    <circle cx="18" cy="18" r="3" />
                    <ellipse cx="12" cy="5" rx="9" ry="3" />
                  </svg>
                  Manage vaults
                {:else}
                  <ShieldCheck aria-hidden="true" data-icon="inline-start" />
                  Account &amp; security
                {/if}
              </Button.Root>
            {/if}
            <Button.Root
              variant="ghost"
              size="sm"
              onclick={() => void logout()}
            >
              <LogOut />
              Sign out
            </Button.Root>
            {#if prepared}
              <Button.Root
                type="button"
                variant="secondary"
                size="icon-lg"
                class="web-vault-launcher__close"
                aria-label={`Return to ${prepared.profile.name}`}
                title={`Return to ${prepared.profile.name}`}
                onclick={returnToWorkspace}
              >
                <X />
              </Button.Root>
            {/if}
          </div>
        </header>

        {#if $pwaState.canInstall && pwa}
          <Button.Root
            variant="outline"
            size="sm"
            onclick={() => void pwa?.install()}
            >Install Lapis Server</Button.Root
          >
        {/if}

        {#if errorMessage}
          <div class="server-alert server-alert--error" role="alert">
            {errorMessage}
          </div>
        {:else if actionMessage}
          <div class="server-alert" role="status">
            {actionMessage}
          </div>
        {/if}

        <div class="vault-launcher-body">
          <div class="server-vault-grid" inert={showAccountSecurityPage}>
            <Card.Root class="server-vault-card">
              <Card.Header>
                <Card.Title class="server-card-title">Server Vaults</Card.Title>
                <Card.Description>
                  Create a managed vault, import a local folder, or upload a
                  ZIP.
                </Card.Description>
              </Card.Header>
              <Card.Content class="server-action-list">
                <Button.Root
                  class="web-vault-launcher__action"
                  disabled={activeAction !== null ||
                    !backend.capabilities.manageVaults}
                  onclick={() => {
                    newVaultName = "";
                    createDialogOpen = true;
                  }}
                >
                  <FolderPlus />
                  <span class="server-action-copy">
                    <span class="server-action-title">Create New Vault</span>
                    <span class="server-action-description">
                      Start a private managed vault on this server.
                    </span>
                  </span>
                </Button.Root>
                <Button.Root
                  variant="outline"
                  class="web-vault-launcher__action"
                  disabled={activeAction !== null ||
                    !canImportDirectory ||
                    !backend.capabilities.transferVaults}
                  onclick={() => void pickImportDirectory()}
                >
                  <FolderInput />
                  <span class="server-action-copy">
                    <span class="server-action-title">Import Folder</span>
                    <span class="server-action-description">
                      {canImportDirectory
                        ? "Choose a folder with the File System Access API."
                        : "Use Import ZIP in browsers without folder access."}
                    </span>
                  </span>
                </Button.Root>
                <Button.Root
                  variant="outline"
                  class="web-vault-launcher__action"
                  disabled={activeAction !== null ||
                    !backend.capabilities.transferVaults}
                  onclick={() => importInput?.click()}
                >
                  <Archive />
                  <span class="server-action-copy">
                    <span class="server-action-title">Import ZIP</span>
                    <span class="server-action-description">
                      Upload a portable vault archive from this device.
                    </span>
                  </span>
                </Button.Root>
                <input
                  bind:this={importInput}
                  class="sr-only"
                  type="file"
                  accept=".zip,application/zip"
                  aria-label="Import ZIP"
                  onchange={() => void importZip()}
                />
              </Card.Content>
            </Card.Root>

            <Card.Root class="server-vault-card">
              <Card.Header>
                <div class="server-card-header-row">
                  <div>
                    <Card.Title class="server-card-title"
                      >Managed Vaults</Card.Title
                    >
                    <Card.Description>
                      Vaults available to this account, including archived
                      projects.
                    </Card.Description>
                  </div>
                  {#if searchableVaults.length > 1}
                    <Button.Root
                      variant="ghost"
                      size="sm"
                      class="server-view-all"
                      onclick={showVaultSearch}
                    >
                      View all
                    </Button.Root>
                  {/if}
                </div>
              </Card.Header>
              <Card.Content class="server-managed-content">
                <ul class="server-vault-list">
                  {#each vaults as vault (vault.id)}
                    <li class="server-vault-row" data-server-vault-row>
                      <button
                        type="button"
                        class="web-vault-launcher__recent server-vault-open"
                        disabled={activeAction !== null ||
                          vault.archivedAt !== undefined}
                        aria-label={vault.archivedAt !== undefined
                          ? `${vault.name} ${vault.role} · archived`
                          : `Open ${vault.name} ${vault.role} · server-hosted`}
                        onclick={() => void openVault(vault)}
                      >
                        <FolderOpen />
                        <span class="server-vault-copy">
                          <span class="server-vault-name">{vault.name}</span>
                          <span class="server-vault-meta">
                            {vault.role}{vault.archivedAt
                              ? " · archived"
                              : " · server-hosted"}
                          </span>
                        </span>
                      </button>
                      <DropdownMenu.Root>
                        <DropdownMenu.Trigger
                          class="server-vault-menu-trigger"
                          aria-label={`Open actions for ${vault.name}`}
                          title={`Open actions for ${vault.name}`}
                          disabled={activeAction !== null}
                        >
                          <Ellipsis />
                        </DropdownMenu.Trigger>
                        <DropdownMenu.Content align="end" side="bottom">
                          {#if vault.archivedAt}
                            <DropdownMenu.Item
                              disabled={!backend.capabilities.manageVaults}
                              onclick={() => void restore(vault)}
                            >
                              <RotateCcw />
                              Restore
                            </DropdownMenu.Item>
                          {:else}
                            <DropdownMenu.Item
                              onclick={() => void openVault(vault)}
                            >
                              <FolderOpen />
                              Open
                            </DropdownMenu.Item>
                            <DropdownMenu.Item
                              disabled={!backend.capabilities.transferVaults}
                              onclick={() => downloadVault(vault)}
                            >
                              <Download />
                              Export ZIP
                            </DropdownMenu.Item>
                            {#if vault.role === "owner"}
                              <DropdownMenu.Item
                                disabled={!backend.capabilities.manageVaults}
                                onclick={() => openRenameDialog(vault)}
                              >
                                <PenLine />
                                Rename...
                              </DropdownMenu.Item>
                              <DropdownMenu.Separator />
                              <DropdownMenu.Item
                                disabled={!backend.capabilities.manageVaults}
                                onclick={() => void archive(vault)}
                              >
                                <Archive />
                                Archive
                              </DropdownMenu.Item>
                            {/if}
                          {/if}
                        </DropdownMenu.Content>
                      </DropdownMenu.Root>
                    </li>
                  {:else}
                    <li class="server-vault-empty">
                      Your managed vaults will appear here after you create or
                      import one.
                    </li>
                  {/each}
                </ul>
              </Card.Content>
            </Card.Root>
          </div>
          {#if showAccountSecurityPage}
            {#if accountSecurityPage}
              {@render accountSecurityPage()}
            {:else if authenticatedUsername}
              <AccountSecurity
                accounts={[
                  {
                    id: authenticatedUsername,
                    label: authenticatedUsername,
                    description: "Current account",
                    security: passkeysEnabled ? backend.security : undefined,
                  },
                ]}
                canUsePasskeys={passkeysAvailable}
              />
            {/if}
          {/if}
        </div>
      </div>
    </section>
  {/if}
</div>

<Dialog.Root bind:open={vaultDialogOpen}>
  <Dialog.Content class="web-vault-launcher__command" showCloseButton={false}>
    <Dialog.Header class="sr-only">
      <Dialog.Title>Managed Vaults</Dialog.Title>
      <Dialog.Description
        >Search and open a managed server vault</Dialog.Description
      >
    </Dialog.Header>
    <CommandView.Root shouldFilter={false}>
      <CommandView.Input
        bind:value={vaultQuery}
        placeholder="Search managed vaults..."
        aria-label="Search managed vaults"
        autocomplete="off"
        spellcheck="false"
      />
      <CommandView.List aria-label="Managed vaults">
        <CommandView.Empty>No matching managed vaults</CommandView.Empty>
        {#if filteredVaults.length > 0}
          <CommandView.Group>
            {#each filteredVaults as vault (vault.id)}
              <CommandView.Item
                value={`${vault.name} ${vault.id} ${vault.role}`}
                onSelect={() => void openVaultFromSearch(vault)}
              >
                <CommandView.ItemIcon>
                  <Search />
                </CommandView.ItemIcon>
                <CommandView.ItemLabel>{vault.name}</CommandView.ItemLabel>
                <CommandView.ItemDescription>
                  {vault.role} · server-hosted
                </CommandView.ItemDescription>
              </CommandView.Item>
            {/each}
          </CommandView.Group>
        {/if}
      </CommandView.List>
    </CommandView.Root>
  </Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={createDialogOpen}>
  <Dialog.Content class="web-vault-launcher__settings">
    <Dialog.Header>
      <Dialog.Title>New Server Vault</Dialog.Title>
      <Dialog.Description>
        Choose the name shown in this account's managed vault list.
      </Dialog.Description>
    </Dialog.Header>
    <form
      class="server-dialog-form"
      onsubmit={(event) => {
        event.preventDefault();
        void createVault();
      }}
    >
      <label class="server-field">
        <span>Name</span>
        <Input.Root bind:value={newVaultName} autocomplete="off" autofocus />
      </label>
      <Dialog.Footer>
        <Button.Root
          type="button"
          variant="ghost"
          onclick={() => (createDialogOpen = false)}
        >
          Cancel
        </Button.Root>
        <Button.Root
          type="submit"
          disabled={!newVaultName.trim() || activeAction !== null}
        >
          Create vault
        </Button.Root>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={directoryDialogOpen}>
  <Dialog.Content class="web-vault-launcher__settings">
    <Dialog.Header>
      <Dialog.Title>Import Server Vault</Dialog.Title>
      <Dialog.Description>
        Upload a managed copy of {directoryHandle?.name ??
          "the selected folder"}.
      </Dialog.Description>
    </Dialog.Header>
    <form
      class="server-dialog-form"
      onsubmit={(event) => {
        event.preventDefault();
        void importDirectory();
      }}
    >
      <label class="server-field">
        <span>Name</span>
        <Input.Root
          bind:value={directoryVaultName}
          autocomplete="off"
          autofocus
        />
      </label>
      {#if importProgress}
        <p class="server-import-progress" role="status">{importProgress}</p>
      {/if}
      <Dialog.Footer>
        <Button.Root
          type="button"
          variant="ghost"
          disabled={activeAction !== null}
          onclick={() => (directoryDialogOpen = false)}
        >
          Cancel
        </Button.Root>
        <Button.Root
          type="submit"
          disabled={!directoryVaultName.trim() || activeAction !== null}
        >
          Import vault
        </Button.Root>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={renameDialogOpen}>
  <Dialog.Content class="web-vault-launcher__settings">
    <Dialog.Header>
      <Dialog.Title>Rename vault</Dialog.Title>
      <Dialog.Description
        >Update the managed vault name for this account.</Dialog.Description
      >
    </Dialog.Header>
    <form
      class="server-dialog-form"
      onsubmit={(event) => {
        event.preventDefault();
        void renameVault();
      }}
    >
      <label class="server-field">
        <span>Name</span>
        <Input.Root bind:value={renameVaultName} autocomplete="off" autofocus />
      </label>
      <Dialog.Footer>
        <Button.Root
          type="button"
          variant="ghost"
          onclick={() => (renameDialogOpen = false)}
        >
          Cancel
        </Button.Root>
        <Button.Root
          type="submit"
          disabled={!renameVaultName.trim() || activeAction !== null}
        >
          Save
        </Button.Root>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<style>
  .launcher-copy-views {
    display: grid;
  }

  .launcher-copy-views > div {
    grid-area: 1 / 1;
  }

  .vault-launcher-body {
    position: relative;
    min-width: 0;
  }

  .launcher-copy-views > [inert],
  .vault-launcher-body > [inert] {
    visibility: hidden;
  }

  .server-launcher-actions :global(.account-security-switch) {
    inline-size: 11rem;
    justify-content: center;
  }
</style>
