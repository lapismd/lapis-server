<script lang="ts">
  import {
    AppBridge,
    PostMessageTransport,
  } from "@modelcontextprotocol/ext-apps/app-bridge";
  import { onMount } from "svelte";
  import notesHtml from "../../.local/mcp-apps-dist/notes.html?raw";
  const taskBundles = import.meta.glob<string>(
    "../../.local/mcp-apps-dist/providers/tasks/tasks.html",
    { eager: true, query: "?raw", import: "default" },
  );
  const tasksHtml = Object.values(taskBundles)[0];
  import {
    noteFixture,
    taskFixture,
    type McpAppKind,
  } from "./mcp-app-fixtures";

  let { kind, theme }: { kind: McpAppKind; theme: "light" | "dark" } = $props();
  let frame = $state<HTMLIFrameElement>();
  const resource = $derived(kind === "notes" ? notesHtml : tasksHtml);
  let ready = $state(false);
  let error = $state(false);
  let height = $state(470);
  let bridge: AppBridge | undefined;
  const title = $derived(kind === "notes" ? "Lapis Notes" : "Lapis Tasks");

  $effect(() => {
    const color = theme;
    if (bridge && ready) void bridge.sendHostContextChange({ theme: color });
  });

  onMount(() => {
    const html = resource;
    if (!html) return;
    const fixture = kind === "notes" ? noteFixture() : taskFixture();
    const iframe = frame;
    const target = iframe?.contentWindow;
    if (!iframe || !target) {
      error = true;
      return;
    }
    let live = true;
    const host = new AppBridge(
      null,
      { name: "Storybook MCP host", version: "1.0.0" },
      { serverTools: {} },
      { hostContext: { theme } },
    );
    bridge = host;
    host.oncalltool = async ({ name, arguments: input }) =>
      fixture.call(name, input ?? {});
    host.onsizechange = ({ height: suggested }) => {
      if (suggested)
        height = Math.min(740, Math.max(120, Math.ceil(suggested) + 4));
    };
    host.oninitialized = () => {
      void (async () => {
        await host.sendToolInput({ arguments: fixture.args });
        await host.sendToolResult(fixture.result);
        if (live) ready = true;
      })().catch(() => {
        if (live) error = true;
      });
    };
    void host
      .connect(new PostMessageTransport(target, target))
      .then(() => {
        if (live) iframe.srcdoc = html;
      })
      .catch(() => {
        if (live) error = true;
      });
    return () => {
      live = false;
      void host.close().catch(() => {});
    };
  });
</script>

<main
  class="conversation"
  class:tasks-conversation={kind === "tasks"}
  aria-label="Example external agent conversation"
>
  <header>
    <h1>External agent conversation</h1>
    <p>Storybook preview · Changes use sample data</p>
  </header>
  <p class="message user">
    {kind === "notes"
      ? "Show me the team offsite note."
      : "Show me the tasks for the team offsite."}
  </p>
  <section class="message assistant" aria-label="Assistant response">
    <p>
      {kind === "notes"
        ? "I found the offsite note in Lapis."
        : "Here are the offsite tasks from Lapis."}
    </p>
    <div
      class="app"
      data-testid="mcp-app"
      data-ready={ready}
      data-unavailable={!resource}
    >
      {#if error}<p role="alert">The MCP App preview could not load.</p>{/if}
      {#if resource}
        <iframe
          bind:this={frame}
          {title}
          sandbox="allow-scripts allow-forms"
          style:height={`${height}px`}
        ></iframe>
      {:else}
        <p role="status">
          The installed Tasks plugin does not contribute an MCP App.
        </p>
      {/if}
    </div>
  </section>
</main>

<style>
  .conversation {
    max-inline-size: 48rem;
    margin-inline: auto;
    padding: clamp(1rem, 4vw, 2rem);
    color: var(--ui-text-primary, CanvasText);
    font:
      14px/1.5 system-ui,
      sans-serif;
  }
  .tasks-conversation {
    max-inline-size: 68rem;
  }
  h1 {
    font-size: 1rem;
    margin: 0;
  }
  p {
    margin: 0;
  }
  header {
    margin-block-end: 1.5rem;
  }
  .message {
    margin-block: 0.75rem;
  }
  .user {
    inline-size: fit-content;
    margin-inline-start: auto;
    padding: 0.6rem 0.8rem;
    border-radius: 0.85rem;
    background: var(--ui-bg-secondary, ButtonFace);
  }
  .app {
    margin-block-start: 0.65rem;
    border: 1px solid var(--ui-border, #8885);
    border-radius: 0.75rem;
    overflow: hidden;
  }
  iframe {
    display: block;
    width: 100%;
    border: 0;
  }
</style>
