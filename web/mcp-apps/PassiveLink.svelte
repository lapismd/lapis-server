<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    children,
    ref = $bindable(null),
  }: {
    children?: Snippet;
    ref?: HTMLElement | null;
  } = $props();
</script>

<span bind:this={ref}>{@render children?.()}</span>
