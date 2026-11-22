<script lang="ts">
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Plug from "@lucide/svelte/icons/plug";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import UserRound from "@lucide/svelte/icons/user-round";
  import * as Alert from "@lapismd/design-core/shadcn/alert";
  import ScopePicker, {
    maxCredentialVaults,
  } from "../content-access/ScopePicker.svelte";
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
  let error = $state("");
  let busy = $state(false);
  const scopeDescriptions: Record<string, string> = {
    "notes:read": "Search notes and read frontmatter",
    "notes:write": "Create, replace, and edit frontmatter in Markdown notes",
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
          <ScopePicker
            label="Vaults"
            searchLabel="Search vaults"
            choices={vaultChoices}
            selected={selectedVaults}
            onChange={(ids) => {
              selectedVaults = ids;
            }}
            limit={maxCredentialVaults}
            disabled={busy}
          />
        </div>
        {#if permissionChoices.length}
          <div class="consent-section">
            <div class="consent-section-label">
              <ShieldCheck aria-hidden="true" />
              <div>
                <h2>Additional permissions</h2>
                <p>Choose the optional actions this application can take.</p>
              </div>
            </div>
            <ScopePicker
              label="Additional permissions"
              searchLabel="Search permissions"
              choices={permissionChoices}
              selected={selectedPermissions}
              onChange={(ids) => {
                selectedPermissions = ids;
              }}
              disabled={busy}
            />
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

  .consent-identity {
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

  .consent-surface :global(.consent-notice) {
    --ui-alert-background: color-mix(in srgb, var(--warning) 10%, var(--card));
    border-color: color-mix(in srgb, var(--warning) 25%, var(--border));
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
