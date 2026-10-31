<script lang="ts" module>
  const disposers = new Set<() => Promise<void>>();
  export async function disposeServerHostFixtures() {
    for (const dispose of disposers) await dispose();
    disposers.clear();
  }
</script>

<script lang="ts">
  import { onMount, tick } from "svelte";
  import ServerVaultHost from "../../web/src/ServerVaultHost.svelte";
  import { releaseHostedFixture } from "./hosted-backend";
  import type { HostedBackend } from "../../web/src/hosted/backend";
  let { backend }: { backend: HostedBackend } = $props();
  let host: { dispose(): Promise<void> } | undefined = $state();
  onMount(() => {
    disposers.add(async () => {
      releaseHostedFixture(backend);
      await tick();
      await host?.dispose();
    });
  });
</script>

<div class="server-host-fixture">
  <ServerVaultHost
    bind:this={
      () => host,
      (component) => {
        if (component) host = component;
      }
    }
    {backend}
  />
</div>

<style>
  .server-host-fixture {
    width: 100%;
    height: 100dvh;
    min-height: 0;
    overflow: hidden;
  }
</style>
