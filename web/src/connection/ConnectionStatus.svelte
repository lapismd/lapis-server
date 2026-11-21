<script lang="ts">
  import * as Popover from "@lapismd/design-core/shadcn/popover";
  import * as Button from "@lapismd/design-core/shadcn/button";
  import Wifi from "@lucide/svelte/icons/wifi";
  import WifiOff from "@lucide/svelte/icons/wifi-off";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Lock from "@lucide/svelte/icons/lock";
  import { readable } from "svelte/store";
  import type { OfflineWorkspace } from "../offline/workspace";
  import type { ServerPwa } from "../pwa/registration";
  import type { HostedConnection } from "./connection";

  let {
    connection,
    signIn,
    offline,
    enableOffline,
    browseOffline,
    canBrowseOffline = true,
    pwa,
    updatePwa,
  }: {
    connection: HostedConnection;
    signIn?: () => void;
    offline?: OfflineWorkspace;
    enableOffline?: () => Promise<void>;
    browseOffline?: () => void;
    canBrowseOffline?: boolean;
    pwa?: ServerPwa;
    updatePwa?: () => Promise<void>;
  } = $props();

  const empty = readable({ grant: undefined, notes: [] });
  const offlineState = $derived(offline ?? empty);
  const emptyPwa = readable({ canInstall: false, updateAvailable: false });
  const pwaState = $derived(pwa ?? emptyPwa);
  let problem = $state("");
  let busy = $state(false);

  async function perform(action: () => Promise<void>) {
    busy = true;
    problem = "";
    try {
      await action();
    } catch (error) {
      problem = error instanceof Error ? error.message : String(error);
    } finally {
      busy = false;
    }
  }

  const names = {
    connected: "Connected",
    offline: "Offline",
    reconnecting: "Reconnecting",
    "sign-in": "Sign-in required",
    "update-required": "Update required",
  };
  const label = $derived(names[$connection.phase]);
  const Icon = $derived(
    $connection.phase === "connected"
      ? Wifi
      : $connection.phase === "offline"
        ? WifiOff
        : $connection.phase === "reconnecting"
          ? RefreshCw
          : Lock,
  );
</script>

<Popover.Root>
  <Popover.Trigger>
    {#snippet child({ props })}
      <button
        {...props}
        type="button"
        class="connection-trigger"
        aria-label={`Server connection: ${label}`}
        title={label}
      >
        <Icon aria-hidden="true" size={14} />
      </button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Content side="top" align="end" sideOffset={6} collisionPadding={8}>
    <section class="connection-details" aria-label="Server connection details">
      <strong>Server connection</strong>
      <dl>
        <dt>Status</dt>
        <dd>{label}</dd>
        <dt>Last contact</dt>
        <dd>
          {$connection.lastContact
            ? new Date($connection.lastContact).toLocaleString()
            : "Unavailable"}
        </dd>
        {#if $offlineState.grant}
          <dt>Cached notes</dt>
          <dd>{$offlineState.notes.length}</dd>
          <dt>Local drafts</dt>
          <dd>{$offlineState.notes.filter((entry) => entry.draft).length}</dd>
        {/if}
      </dl>
      {#if $connection.phase === "sign-in" && signIn}
        <Button.Root variant="outline" size="sm" onclick={signIn}
          >Sign in</Button.Root
        >
      {:else if $connection.phase === "offline" || $connection.phase === "reconnecting"}
        <Button.Root
          variant="outline"
          size="sm"
          disabled={$connection.phase === "reconnecting"}
          onclick={() => void connection.retry()}>Retry now</Button.Root
        >
      {/if}
      {#if enableOffline && !$offlineState.grant && $connection.phase === "connected"}
        <Button.Root
          variant="outline"
          size="sm"
          disabled={busy}
          onclick={() => enableOffline && void perform(enableOffline)}
          >Keep notes on this device</Button.Root
        >
      {/if}
      {#if browseOffline && canBrowseOffline && $offlineState.grant}
        <Button.Root variant="outline" size="sm" onclick={browseOffline}
          >Browse cached notes and drafts</Button.Root
        >
      {/if}
      {#if $pwaState.canInstall && pwa}
        <Button.Root
          variant="outline"
          size="sm"
          disabled={busy}
          onclick={() => pwa && void perform(pwa.install)}
          >Install Lapis Server</Button.Root
        >
      {/if}
      {#if $pwaState.updateAvailable && updatePwa}
        <Button.Root
          variant="outline"
          size="sm"
          disabled={busy || $connection.phase !== "connected"}
          onclick={() => updatePwa && void perform(updatePwa)}
          >Apply app update</Button.Root
        >
      {/if}
      {#if problem}<p role="status">{problem}</p>{/if}
    </section>
  </Popover.Content>
</Popover.Root>

<style>
  .connection-trigger {
    display: inline-grid;
    min-width: 1.75rem;
    min-height: 1.5rem;
    place-items: center;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .connection-trigger:hover,
  .connection-trigger[aria-expanded="true"] {
    background: var(--accent);
    color: var(--accent-foreground);
  }
  .connection-details {
    display: grid;
    gap: 0.75rem;
  }
  dl {
    display: grid;
    grid-template-columns: auto auto;
    gap: 0.25rem 0.75rem;
    margin: 0;
    font-size: 0.875rem;
  }
  dt {
    color: var(--muted-foreground);
  }
  dd {
    margin: 0;
  }
</style>
