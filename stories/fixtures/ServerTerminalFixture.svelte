<script lang="ts" module>
  import { createTerminalBackend } from "./terminal-backend";
  const backends = new WeakMap<
    HTMLElement,
    ReturnType<typeof createTerminalBackend>
  >();
  export function terminalReviewFor(canvas: HTMLElement) {
    const owner = canvas.querySelector<HTMLElement>("[data-terminal-review]");
    const backend = owner && backends.get(owner);
    if (!backend) throw Error("Terminal review fixture is not mounted");
    return backend.review;
  }
</script>

<script lang="ts">
  import ServerHostFixture from "./ServerHostFixture.svelte";
  let { backend }: { backend: ReturnType<typeof createTerminalBackend> } =
    $props();
  function own(element: HTMLElement) {
    backends.set(element, backend);
    return () => {
      backends.delete(element);
    };
  }
</script>

<div data-terminal-review {@attach own}>
  <ServerHostFixture {backend} />
</div>
