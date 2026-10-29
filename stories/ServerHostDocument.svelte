<script lang="ts">
  import { onMount } from "svelte";
  import { initializeServerHostDocument } from "../web/src/server-host-document";

  onMount(() => {
    initializeServerHostDocument();
  });
</script>

<p data-testid="server-host-runtime">rust-server</p>
