<script lang="ts">
  import { onMount, untrack } from "svelte";
  import * as Button from "@lapismd/design-core/shadcn/button";
  import * as Textarea from "@lapismd/design-core/shadcn/textarea";
  import { registerOfflineWriter } from "./persistence";
  import type { SavedNote } from "./database";
  import type { DraftRemote, OfflineWorkspace } from "./workspace";

  let {
    initial,
    offline,
    remote,
  }: {
    initial: SavedNote;
    offline: OfflineWorkspace;
    remote?: DraftRemote;
  } = $props();
  let note = $state(untrack(() => initial));
  let text = $state(untrack(() => initial.draft?.text ?? initial.text));
  let error = $state("");
  let feedback = $state("");
  let serverText = $state<string>();
  let saving = $state(false);
  let pending = $state(false);
  let tail = Promise.resolve();

  function update(value: string) {
    text = value;
    pending = true;
    error = "";
    feedback = "";
    tail = tail.then(async () => {
      try {
        note = await offline.stage(note, value);
      } catch (failure) {
        error = failure instanceof Error ? failure.message : String(failure);
      } finally {
        if (value === text) pending = false;
      }
    });
  }

  async function apply() {
    if (!remote || saving) return;
    await tail;
    if (error || !note.draft) return;
    saving = true;
    serverText = undefined;
    try {
      const result = await offline.apply(note, remote);
      if (result === "saved") {
        note = { ...note, text, draft: undefined };
        feedback = "Saved to the server.";
      } else if (result === "missing") {
        feedback =
          "This note is no longer at its original location. Your draft is still on this device.";
      } else {
        feedback =
          "This note changed on the server. Your draft is still on this device.";
        serverText = (await remote.read(note.path))?.text;
      }
    } catch {
      feedback =
        "The save could not be confirmed. Your draft is still on this device.";
    } finally {
      saving = false;
    }
  }

  onMount(() =>
    registerOfflineWriter(async () => {
      await tail;
      if (error) throw Error(error);
    }),
  );
</script>

<div class="offline-note">
  <header>
    <h2>{note.path}</h2>
    <p aria-live="polite">
      {pending
        ? "Saving draft on this device…"
        : note.draft
          ? "Local draft"
          : "Cached note"}
    </p>
  </header>
  <Textarea.Root
    aria-label="Markdown draft"
    value={text}
    disabled={saving}
    oninput={(event) => update(event.currentTarget.value)}
  />
  {#if error}<p class="offline-note-error">{error}</p>{/if}
  {#if feedback}<p role="status">{feedback}</p>{/if}
  {#if serverText !== undefined}
    <details open>
      <summary>Current server version</summary>
      <pre>{serverText}</pre>
    </details>
  {/if}
  {#if remote && note.draft}
    <Button.Root
      onclick={() => void apply()}
      disabled={pending || saving || !!error}
    >
      Save draft to server
    </Button.Root>
  {/if}
</div>

<style>
  .offline-note {
    display: grid;
    align-content: start;
    gap: 0.75rem;
    min-width: 0;
  }
  h2,
  p {
    margin: 0;
    overflow-wrap: anywhere;
  }
  h2 {
    font-size: 1.125rem;
  }
  header p,
  [role="status"] {
    color: var(--muted-foreground);
    font-size: 0.875rem;
  }
  :global(.offline-note textarea) {
    min-height: 14rem;
    font-family: var(--ui-workspace-font-mono, monospace);
  }
  .offline-note-error {
    color: var(--destructive);
  }
  pre {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
