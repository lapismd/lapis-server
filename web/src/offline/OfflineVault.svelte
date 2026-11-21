<script lang="ts">
  import * as Button from "@lapismd/design-core/shadcn/button";
  import * as Input from "@lapismd/design-core/shadcn/input";
  import ConnectionStatus from "../connection/ConnectionStatus.svelte";
  import type { HostedConnection } from "../connection/connection";
  import OfflineNoteEditor from "./OfflineNoteEditor.svelte";
  import type { DraftRemote, OfflineWorkspace } from "./workspace";

  let {
    offline,
    connection,
    remote,
    resume,
    signIn,
  }: {
    offline: OfflineWorkspace;
    connection: HostedConnection;
    remote?: DraftRemote;
    resume?: (path?: string) => void;
    signIn: () => void;
  } = $props();

  let query = $state("");
  const selected = $derived($offline.grant?.selection);
  const vault = $derived(
    $offline.grant?.vaults.find((entry) => entry.id === selected?.vault),
  );
  const note = $derived(
    $offline.notes.find((entry) => entry.path === selected?.path),
  );
  const visible = $derived(
    $offline.notes.filter(
      ({ path, text, draft }) =>
        !query ||
        `${path}\n${draft?.text ?? text}`
          .toLowerCase()
          .includes(query.toLowerCase()),
    ),
  );
  let clearing = $state(false);
  let clearError = $state("");

  async function clear() {
    try {
      await offline.clear();
    } catch {
      clearError = "Offline data could not be cleared. Try again.";
    }
  }
</script>

<div class="offline-workspace">
  <header>
    <div>
      <h1>{vault?.name ?? "Lapis Server"}</h1>
      <p>Cached notes</p>
    </div>
    {#if resume}
      <Button.Root
        variant="outline"
        size="sm"
        onclick={() => resume?.(note?.path)}
      >
        Return to workspace
      </Button.Root>
    {/if}
  </header>
  <div class="offline-main">
    <nav aria-label="Cached notes">
      <Input.Root
        type="search"
        placeholder="Search cached notes"
        aria-label="Search cached notes"
        bind:value={query}
      />
      {#each visible as entry (entry.path)}
        <button
          type="button"
          class="note-row"
          aria-current={entry.path === note?.path ? "true" : undefined}
          onclick={() =>
            void offline.select(entry.account, entry.vault, entry.path)}
        >
          <span>{entry.path}</span>{#if entry.draft}<small>Draft</small>{/if}
        </button>
      {:else}
        <p>
          {query ? "No matching cached notes." : "No notes available offline."}
        </p>
      {/each}
    </nav>
    <main aria-label="Cached note">
      {#if note}
        {#key `${note.account}/${note.vault}/${note.path}`}
          <OfflineNoteEditor initial={note} {offline} {remote} />
        {/key}
      {:else}
        <p>Select a cached note to read or edit.</p>
      {/if}
    </main>
  </div>
  <footer aria-label="Status bar">
    {#if clearing}
      <span>Remove cached notes and local drafts from this device?</span>
      <Button.Root variant="destructive" size="sm" onclick={() => void clear()}
        >Clear data</Button.Root
      >
      <Button.Root variant="ghost" size="sm" onclick={() => (clearing = false)}
        >Cancel</Button.Root
      >
    {:else}
      <Button.Root variant="ghost" size="sm" onclick={() => (clearing = true)}
        >Clear offline data</Button.Root
      >
    {/if}
    {#if clearError}<span>{clearError}</span>{/if}
    <div class="connection">
      <ConnectionStatus {connection} {signIn} {offline} />
    </div>
  </footer>
</div>

<style>
  .offline-workspace {
    display: grid;
    height: 100%;
    grid-template-rows: auto minmax(0, 1fr) auto;
    background: var(--ui-workspace-background);
    color: var(--ui-workspace-foreground);
  }
  header,
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    background: var(--ui-workspace-panel);
  }
  header {
    padding-inline-start: max(
      0.75rem,
      var(--ui-workspace-window-controls-overlay-inline-start, 0px)
    );
    padding-inline-end: max(
      0.75rem,
      var(--ui-workspace-window-controls-overlay-inline-end, 0px)
    );
    min-height: var(--ui-workspace-window-controls-overlay-height, 0px);
    border-bottom: 1px solid var(--border);
  }
  footer {
    border-top: 1px solid var(--border);
  }
  .connection {
    margin-inline-start: auto;
  }
  h1,
  p {
    margin: 0;
  }
  h1 {
    font-size: 1rem;
  }
  p {
    color: var(--muted-foreground);
    font-size: 0.875rem;
  }
  .offline-main {
    display: grid;
    grid-template-columns: minmax(12rem, 16rem) minmax(0, 1fr);
    min-height: 0;
  }
  nav,
  main {
    display: grid;
    align-content: start;
    gap: 0.5rem;
    padding: 0.75rem;
    overflow: auto;
  }
  nav {
    border-inline-end: 1px solid var(--border);
    background: var(--ui-workspace-panel);
  }
  .note-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.5rem;
    text-align: start;
    overflow-wrap: anywhere;
    color: inherit;
    background: transparent;
    border: 0;
    border-radius: var(--radius-sm);
    padding: 0.375rem;
    cursor: pointer;
  }
  .note-row:hover,
  .note-row[aria-current="true"] {
    background: var(--accent);
    color: var(--accent-foreground);
  }
  @media (max-width: 40rem) {
    .offline-main {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(5rem, 26%) minmax(0, 1fr);
    }
    nav {
      border-inline-end: 0;
      border-bottom: 1px solid var(--border);
    }
  }
</style>
