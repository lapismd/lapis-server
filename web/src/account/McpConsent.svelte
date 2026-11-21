<script lang="ts">
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
    vaults,
    transport = serverTransport,
  }: {
    requestId: string;
    vaults: HostedVault[];
    transport?: ConsentTransport;
  } = $props();
  let request = $state<Request>();
  let selectedVaults = $state<string[]>([]);
  let selectedPermissions = $state<string[]>([]);
  let error = $state("");
  let busy = $state(false);
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
            "This connection request expired. Start again from your MCP client.";
      });
    return () => {
      alive = false;
    };
  });

  async function decide(approve: boolean) {
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
        "Lapis could not finish this connection. Start again from your MCP client.";
    } finally {
      busy = false;
    }
  }
</script>

<section class="server-centered-surface">
  <Card.Root class="server-status-card">
    <Card.Header>
      <h1 class="server-status-title">Connect an MCP client</h1>
      {#if request}
        <Card.Description
          >{request.client_name} is requesting access to Lapis Server.</Card.Description
        >
      {/if}
    </Card.Header>
    <Card.Content>
      {#if request}
        <dl>
          <dt>Client ID</dt>
          <dd>{request.client_id}</dd>
          <dt>Returns to</dt>
          <dd>{request.callback_host}</dd>
        </dl>
        <fieldset disabled={busy}>
          <legend>Choose the vaults this client can read</legend>
          {#each vaults.filter((vault) => vault.archivedAt === undefined) as vault (vault.id)}
            <label
              ><input
                type="checkbox"
                value={vault.id}
                bind:group={selectedVaults}
              />
              {vault.name}</label
            >
          {/each}
        </fieldset>
        {#if request.permissions.length}
          <fieldset disabled={busy}>
            <legend>Optional plugin permissions</legend>
            {#each request.permissions as permission (permission)}
              <label
                ><input
                  type="checkbox"
                  value={permission}
                  bind:group={selectedPermissions}
                />
                {permission}</label
              >
            {/each}
          </fieldset>
        {/if}
      {/if}
      {#if error}<p class="server-error" role="alert">{error}</p>{/if}
    </Card.Content>
    <Card.Footer class="actions">
      {#if request}
        <Button.Root
          variant="outline"
          disabled={busy}
          onclick={() => void decide(false)}>Deny</Button.Root
        >
        <Button.Root
          disabled={busy || selectedVaults.length === 0}
          onclick={() => void decide(true)}>Allow access</Button.Root
        >
      {:else if error}
        <a href="/">Return to Lapis</a>
      {/if}
    </Card.Footer>
  </Card.Root>
</section>

<style>
  dt {
    font-weight: 600;
  }
  dd {
    overflow-wrap: anywhere;
    margin-inline-start: 0;
  }
  fieldset {
    display: grid;
    gap: 0.4rem;
    margin-block: 0.75rem;
  }
  :global(.actions) {
    gap: 0.5rem;
  }
</style>
