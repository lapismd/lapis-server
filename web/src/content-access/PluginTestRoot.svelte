<script lang="ts">
  import { untrack } from "svelte";

  // Test-owned App construction needs a real Svelte component lifecycle.
  let { create }: { create: () => void } = $props();
  untrack(() => create());
</script>
