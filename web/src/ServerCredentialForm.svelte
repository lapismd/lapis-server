<script lang="ts">
  import { onMount } from "svelte";
  import { App, MemoryAppDatabase, MemoryVaultAdapter } from "@lapis-notes/api";
  import { WorkspaceShell } from "@lapis-notes/workspace";
  import * as Button from "@lapismd/design-core/shadcn/button";
  import * as Card from "@lapismd/design-core/shadcn/card";
  import * as Input from "@lapismd/design-core/shadcn/input";

  const adapter = new MemoryVaultAdapter({
    "Notes/Welcome.md":
      "# Welcome\n\nOpen this note through the hosted server.\n",
  });
  const app = new App({
    version: "0.1.0",
    configPath: ".lapis/app.json",
    adapter,
    appDatabase: new MemoryAppDatabase("hosted-credential-ui"),
    markdownRenderer: async () => {},
  });

  let ready = $state(false);
  let username = $state("");
  let password = $state("");

  onMount(() => {
    void (async () => {
      await app.vault.load();
      await app.configuration.load();
      await app.workspace.loadLayout();
      ready = true;
    })();
  });
</script>

<section class="credential-shell" data-testid="server-credential-shell">
  {#if ready}
    <div class="credential-workspace" data-testid="server-credential-workspace">
      <WorkspaceShell
        {app}
        displayMode="desktop"
        workspaceLabel="Hosted Notes"
      />
    </div>
  {/if}
  <form
    class="credential-form"
    data-testid="server-credential-form"
    onsubmit={(event) => event.preventDefault()}
  >
    <Card.Root>
      <Card.Header>
        <Card.Title>Sign in</Card.Title>
        <Card.Description>
          Open your managed Lapis vaults from this server.
        </Card.Description>
      </Card.Header>
      <Card.Content>
        <label>
          Username
          <Input.Root
            name="username"
            autocomplete="username"
            bind:value={username}
          />
        </label>
        <label>
          Password
          <Input.Root
            name="password"
            type="password"
            autocomplete="current-password"
            bind:value={password}
          />
        </label>
        <Button.Root type="submit">Sign in</Button.Root>
      </Card.Content>
    </Card.Root>
  </form>
</section>

<style>
  .credential-shell {
    position: relative;
    height: 100%;
    min-height: 100vh;
    padding: 0;
    overflow: hidden;
  }

  .credential-workspace {
    height: 100%;
    min-height: 100vh;
  }

  .credential-form {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    margin: 0;
    padding: 0;
  }
</style>
