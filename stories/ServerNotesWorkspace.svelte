<script lang="ts">
  import { onMount } from "svelte";
  import { App, MemoryAppDatabase, MemoryVaultAdapter } from "@lapis-notes/api";
  import { WorkspaceShell } from "@lapis-notes/workspace";

  const shellExport = WorkspaceShell;
  const adapter = new MemoryVaultAdapter({
    "Notes/Welcome.md":
      "# Welcome\n\nOpen this note through the hosted server.\n",
  });
  const app = new App({
    version: "0.1.0",
    configPath: ".lapis/app.json",
    adapter,
    appDatabase: new MemoryAppDatabase("hosted-notes-ui"),
    markdownRenderer: async () => {},
  });

  let ready = $state(false);
  let failure = $state("");

  onMount(() => {
    void (async () => {
      try {
        await app.vault.load();
        await app.configuration.load();
        await app.workspace.loadLayout();
        ready = true;
      } catch (error) {
        failure = error instanceof Error ? error.message : String(error);
      }
    })();
  });
</script>

<section
  data-testid="hosted-notes-workspace"
  data-shell-export={typeof shellExport}
>
  {#if failure}
    <p data-testid="hosted-notes-workspace-error">{failure}</p>
  {:else if ready}
    <div data-testid="hosted-notes-workspace-shell">
      <WorkspaceShell
        {app}
        displayMode="desktop"
        workspaceLabel="Hosted Notes"
      />
    </div>
  {:else}
    <p data-testid="hosted-notes-workspace-status">Opening Lapis Notes</p>
  {/if}
</section>
