<script lang="ts">
  import AccountSecurity, {
    type SecurityAccount,
  } from "../../web/src/account/AccountSecurity.svelte";
  import ServerHostFixture from "./ServerHostFixture.svelte";
  import {
    createFixturePasskeyManagement,
    createHostedFixture,
  } from "./hosted-backend";

  const { backend } = createHostedFixture("passkey-management");
  const created = Date.UTC(2026, 0, 1);
  const accounts: [SecurityAccount, SecurityAccount] = [
    {
      id: "owner",
      label: "Owner",
      description: "Current account",
      security: createFixturePasskeyManagement([
        {
          id: "laptop",
          label: "MacBook Pro",
          created_at: created,
          last_used_at: created,
        },
        {
          id: "key",
          label: "Security key",
          created_at: created,
          last_used_at: null,
        },
      ]),
    },
    {
      id: "example",
      label: "Example account",
      description: "Preview account",
      security: createFixturePasskeyManagement(),
    },
  ];
</script>

<ServerHostFixture {backend} restoreVaultId="notes">
  {#snippet accountSecurityPage()}
    <AccountSecurity {accounts} canUsePasskeys />
  {/snippet}
</ServerHostFixture>
