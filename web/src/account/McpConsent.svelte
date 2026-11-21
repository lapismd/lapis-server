<script lang="ts">
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import Check from "@lucide/svelte/icons/check";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Plug from "@lucide/svelte/icons/plug";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import UserRound from "@lucide/svelte/icons/user-round";
  import X from "@lucide/svelte/icons/x";
  import * as Alert from "@lapismd/design-core/shadcn/alert";
  import * as CommandView from "@lapismd/design-core/shadcn/command-view";
  import serverLogo from "../assets/lapis-logo.svg?url";
  import { onMount } from "svelte";
  import * as Button from "@lapismd/design-core/shadcn/button";
  import * as Card from "@lapismd/design-core/shadcn/card";
  import { RustHttpClient } from "../hosted/rust/client";
  import type { HostedVault } from "../hosted/backend";

  type Request = {
    client_name: string;
    client_id: string;
    callback_host: string;
    permissions: string[];
  };
  type Decision = {
    approve: boolean;
    vault_ids: string[];
    permissions: string[];
  };
  type ConsentTransport = {
    load(path: string): Promise<Request>;
    decide(path: string, decision: Decision): Promise<void>;
  };
  const host = new RustHttpClient("");
  const serverTransport: ConsentTransport = {
    async load(path) {
      await host.current();
      return host.json<Request>(path);
    },
    async decide(path, decision) {
      const { redirect } = await host.json<{ redirect: string }>(
        path,
        "POST",
        decision,
      );
      window.location.assign(redirect);
    },
  };
  let {
    requestId,
    username,
    vaults,
    transport = serverTransport,
  }: {
    requestId: string;
    username?: string | null;
    vaults: HostedVault[];
    transport?: ConsentTransport;
  } = $props();
  let request = $state<Request>();
  let selectedVaults = $state<string[]>([]);
  let selectedPermissions = $state<string[]>([]);
  let vaultSearch = $state("");
  let permissionSearch = $state("");
  let error = $state("");
  let busy = $state(false);
  const maxVaults = 20;
  type Choice = { id: string; label: string; description?: string };
  const scopeDescriptions: Record<string, string> = {
    "skills:read": "List available and managed skills",
    "skills:write": "Install and update packaged skills",
    "tasks:read": "List, read, and inspect tasks",
    "tasks:write": "Run actions such as completing or reopening tasks",
  };
  const vaultChoices = $derived(
    vaults
      .filter((vault) => vault.archivedAt === undefined)
      .map((vault) => ({ id: vault.id, label: vault.name })),
  );
  const permissionChoices = $derived(
    (request?.permissions ?? []).map((scope) => ({
      id: scope,
      label: scope,
      description: scopeDescriptions[scope],
    })),
  );

  function toggle(selected: string[], id: string) {
    return selected.includes(id)
      ? selected.filter((entry) => entry !== id)
      : [...selected, id];
  }
  const path = $derived(
    `/content-access/mcp/consent/${encodeURIComponent(requestId)}`,
  );

  onMount(() => {
    let alive = true;
    void transport
      .load(path)
      .then((details) => {
        if (alive) request = details;
      })
      .catch(() => {
        if (alive)
          error =
            "This authorization request expired. Start again from the application.";
      });
    return () => {
      alive = false;
    };
  });

  async function decide(approve: boolean) {
    if (busy || !request || (approve && !selectedVaults.length)) return;
    busy = true;
    error = "";
    try {
      await transport.decide(path, {
        approve,
        vault_ids: approve ? selectedVaults : [],
        permissions: approve ? selectedPermissions : [],
      });
    } catch {
      error =
        "Lapis could not complete authorization. Start again from the application.";
    } finally {
      busy = false;
    }
  }
</script>

{#snippet picker(
  label: string,
  searchLabel: string,
  choices: Choice[],
  selected: string[],
  onToggle: (id: string) => void,
  query: string,
  onSearch: (value: string) => void,
  limit?: number,
)}
  <div class="consent-picker">
    {#if selected.length}
      <ul class="selection-summary" aria-label={`${label} selected`}>
        {#each choices.filter( (choice) => selected.includes(choice.id), ) as choice (choice.id)}
          <li>
            <Button.Root
              type="button"
              size="sm"
              variant="secondary"
              disabled={busy}
              aria-label={`Remove ${choice.label}`}
              onclick={() => onToggle(choice.id)}
            >
              {choice.label}
              <X aria-hidden="true" data-icon="inline-end" />
            </Button.Root>
          </li>
        {/each}
      </ul>
    {/if}
    <CommandView.Root {label} shouldFilter={false}>
      <CommandView.Input
        bind:value={() => query, onSearch}
        aria-label={searchLabel}
        placeholder={`${searchLabel}…`}
        disabled={busy}
        autocomplete="off"
      />
      <CommandView.List aria-label={label} aria-multiselectable="true">
        <CommandView.Empty>No matching options.</CommandView.Empty>
        <CommandView.Group>
          {#each choices.filter( (choice) => `${choice.label} ${choice.description ?? ""}`
                .toLowerCase()
                .includes(query.trim().toLowerCase()), ) as choice (choice.id)}
            {@const checked = selected.includes(choice.id)}
            <CommandView.Item
              value={choice.id}
              keywords={[choice.label, choice.description ?? ""]}
              disabled={busy ||
                (!checked && limit !== undefined && selected.length >= limit)}
              onSelect={() => onToggle(choice.id)}
            >
              {#snippet child({ props })}
                <div {...props} aria-selected={checked} class="consent-option">
                  <CommandView.ItemIcon>
                    <span
                      class="selection-mark"
                      class:checked
                      aria-hidden="true"
                    >
                      {#if checked}<Check />{/if}
                    </span>
                  </CommandView.ItemIcon>
                  <CommandView.ItemLabel>{choice.label}</CommandView.ItemLabel>
                  {#if choice.description}
                    <CommandView.ItemDescription
                      >{choice.description}</CommandView.ItemDescription
                    >
                  {/if}
                </div>
              {/snippet}
            </CommandView.Item>
          {/each}
        </CommandView.Group>
      </CommandView.List>
      {#if limit !== undefined && selected.length >= limit}
        <CommandView.Footer
          >Up to {limit} vaults can be shared.</CommandView.Footer
        >
      {/if}
    </CommandView.Root>
  </div>
{/snippet}

<section class="server-centered-surface consent-surface">
  <Card.Root class="server-login-card consent-card">
    <Card.Header class="consent-header">
      <img src={serverLogo} alt="" class="server-login-logo consent-logo" />
      <div>
        <p class="server-eyebrow">Lapis Notes</p>
        <h1 class="server-login-title">Authorize application</h1>
        {#if request}
          <Card.Description
            >Review the access this application is requesting.</Card.Description
          >
        {/if}
      </div>
    </Card.Header>
    <Card.Content class="consent-content">
      {#if request}
        <dl class="consent-identity">
          <div>
            <dt>Requesting application</dt>
            <dd><Plug aria-hidden="true" />{request.client_name}</dd>
          </div>
          <div>
            <dt>Returns to</dt>
            <dd>{request.callback_host}</dd>
          </div>
          {#if username}
            <div>
              <dt>Authorizing as</dt>
              <dd><UserRound aria-hidden="true" />{username}</dd>
            </div>
          {/if}
          <div>
            <dt>Client ID</dt>
            <dd>{request.client_id}</dd>
          </div>
        </dl>

        <dl class="connection-scopes" aria-label="Connection scopes">
          <div>
            <dt><code>mcp:read</code></dt>
            <dd>Read the selected vaults</dd>
          </div>
          <div>
            <dt><code>offline_access</code></dt>
            <dd>Renew access without signing in again</dd>
          </div>
        </dl>

        <div class="consent-section">
          <div class="consent-section-label">
            <FolderOpen aria-hidden="true" />
            <div>
              <h2>Vaults</h2>
              <p>Choose the vaults this application can read.</p>
            </div>
          </div>
          {@render picker(
            "Vaults",
            "Search vaults",
            vaultChoices,
            selectedVaults,
            (id) => {
              selectedVaults = toggle(selectedVaults, id);
            },
            vaultSearch,
            (value) => {
              vaultSearch = value;
            },
            maxVaults,
          )}
        </div>
        {#if permissionChoices.length}
          <div class="consent-section">
            <div class="consent-section-label">
              <ShieldCheck aria-hidden="true" />
              <div>
                <h2>Plugin permissions</h2>
                <p>Choose the optional actions this application can take.</p>
              </div>
            </div>
            {@render picker(
              "Plugin permissions",
              "Search permissions",
              permissionChoices,
              selectedPermissions,
              (id) => {
                selectedPermissions = toggle(selectedPermissions, id);
              },
              permissionSearch,
              (value) => {
                permissionSearch = value;
              },
            )}
          </div>
        {/if}
        <Alert.Root class="consent-notice" role="note">
          <CircleAlert aria-hidden="true" />
          <Alert.Title>Access is limited to your selection</Alert.Title>
          <Alert.Description>
            You can disconnect this application in Content API and MCP settings.
          </Alert.Description>
        </Alert.Root>
      {/if}
      {#if error}<p class="server-error" role="alert">{error}</p>{/if}
    </Card.Content>
    <Card.Footer class="consent-footer">
      {#if request}
        <div class="consent-actions">
          <Button.Root
            variant="outline"
            disabled={busy}
            onclick={() => void decide(false)}>Deny</Button.Root
          >
          <Button.Root
            disabled={busy || selectedVaults.length === 0}
            onclick={() => void decide(true)}>Allow access</Button.Root
          >
        </div>
      {:else if error}
        <a href="/">Return to Lapis</a>
      {/if}
    </Card.Footer>
  </Card.Root>
</section>

<style>
  .consent-surface {
    align-items: safe center;
  }

  .consent-surface :global(.consent-card) {
    width: min(100%, 49rem);
  }

  .consent-surface :global(.consent-header) {
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: 1rem;
  }

  .consent-logo {
    width: 3.5rem;
    height: 3.5rem;
    border-radius: 1rem;
  }

  .consent-surface :global(.consent-content) {
    display: grid;
    gap: 1rem;
  }

  .consent-identity,
  .consent-picker {
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--background);
  }

  .consent-identity {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    margin: 0;
    padding: 0.875rem 1rem;
    gap: 0.75rem 1rem;
    font-size: 0.875rem;
  }

  dt,
  .consent-section-label p {
    color: var(--muted-foreground);
    font-size: 0.8125rem;
  }

  dd {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    font-weight: 500;
    overflow-wrap: anywhere;
    margin-inline-start: 0;
  }

  dd :global(svg),
  .consent-section-label > :global(svg) {
    width: 1rem;
    height: 1rem;
    flex: none;
  }

  .consent-section {
    display: grid;
    grid-template-columns: minmax(0, 11rem) minmax(0, 1fr);
    align-items: start;
    gap: 1rem;
  }

  .consent-section-label {
    display: flex;
    align-items: flex-start;
    gap: 0.625rem;
  }

  .consent-section-label > :global(svg) {
    margin-block-start: 0.2rem;
  }

  .consent-section-label h2 {
    margin: 0;
    font-size: 0.9375rem;
    font-weight: 600;
  }

  .consent-section-label p {
    margin: 0;
  }

  .connection-scopes {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem 1.25rem;
    margin: 0;
    font-size: 0.8125rem;
  }

  .connection-scopes > div {
    min-width: 0;
  }

  .consent-picker {
    --ui-command-view-list-max-height: 13rem;
  }

  .consent-surface :global(.consent-notice) {
    --ui-alert-background: color-mix(in srgb, var(--warning) 10%, var(--card));
    border-color: color-mix(in srgb, var(--warning) 25%, var(--border));
  }

  .selection-summary {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    list-style: none;
    padding: 0.375rem;
    margin: 0;
    border-block-end: 1px solid var(--border);
  }

  .consent-option {
    align-items: center;
  }

  .selection-mark {
    display: grid;
    place-items: center;
    width: 1rem;
    height: 1rem;
    border: 1px solid var(--input);
    border-radius: 0.25rem;
  }

  .selection-mark.checked {
    border-color: var(--primary);
    background: var(--primary);
    color: var(--primary-foreground);
  }

  .selection-mark :global(svg) {
    width: 0.75rem;
    height: 0.75rem;
  }

  .consent-surface :global(.consent-footer) {
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .consent-actions {
    display: flex;
    gap: 0.5rem;
    margin-inline-start: auto;
  }

  @media (max-width: 42rem) {
    .consent-section {
      grid-template-columns: minmax(0, 1fr);
      gap: 0.5rem;
    }
  }

  @media (max-width: 30rem) {
    .consent-identity {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
