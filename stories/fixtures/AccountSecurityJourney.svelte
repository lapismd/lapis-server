<script lang="ts">
  import AccountSecurityPreview from "./AccountSecurityPreview.svelte";
  import ServerHostFixture from "./ServerHostFixture.svelte";
  import { createHostedFixture } from "./hosted-backend";

  const { backend } = createHostedFixture("passkey-management");
</script>

<ServerHostFixture {backend} restoreVaultId="notes">
  {#snippet accountSecurityPage()}
    <AccountSecurityPreview />
  {/snippet}
</ServerHostFixture>
