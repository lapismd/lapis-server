<script lang="ts">
  import { Button } from "@lapismd/design-core/shadcn/button";
  import { MarkdownPreview } from "@lapismd/mira/preview";
  import { onMount } from "svelte";
  import { z } from "zod";
  import { connectView, toolValue, type ToolBridge } from "./bridge";
  import PassiveLink from "./PassiveLink.svelte";
  import PassiveMedia from "./PassiveMedia.svelte";

  const noteInput = z.object({ vault_id: z.uuid(), path: z.string() });
  const noteValue = z.object({
    kind: z.literal("note"),
    path: z.string(),
    text: z.string(),
    version: z.string(),
  });
  const components = {
    a: PassiveLink,
    img: PassiveMedia,
    audio: PassiveMedia,
    video: PassiveMedia,
    iframe: PassiveMedia,
    embed: PassiveMedia,
    object: PassiveMedia,
    source: null,
    track: null,
  };

  let bridge: ToolBridge;
  let args = $state.raw<z.infer<typeof noteInput>>();
  let note = $state<z.infer<typeof noteValue>>();
  let canCall = $state(false);
  let busy = $state(true);
  let error = $state("");

  onMount(() => {
    bridge = connectView("Lapis Notes", {
      input(value) {
        args = noteInput.safeParse(value).data;
      },
      result(result) {
        try {
          note = toolValue(result, noteValue);
        } catch {
          error = "The saved note could not be read.";
        }
        busy = false;
      },
      ready(available) {
        canCall = available;
      },
      failed(message) {
        error = message;
        busy = false;
      },
    });
    return () => bridge.stop();
  });

  async function refresh() {
    if (busy || !args || !canCall) return;
    busy = true;
    error = "";
    try {
      note = toolValue(await bridge.call("read_note", args), noteValue);
    } catch {
      error = "The note could not be refreshed.";
    } finally {
      busy = false;
    }
  }
</script>

<main>
  <header class="toolbar">
    <h1>{note?.path ?? "Lapis Notes"}</h1>
    {#if canCall && args}
      <Button variant="outline" size="sm" disabled={busy} onclick={refresh}
        >Refresh</Button
      >
    {/if}
  </header>
  {#if error}<p class="message" role="alert">{error}</p>{/if}
  {#if note}
    <MarkdownPreview
      value={note.text}
      sourcePath={note.path}
      htmlPolicy="safe"
      {components}
      dialog={false}
    />
  {:else if busy}
    <p role="status">Reading note…</p>
  {/if}
</main>
