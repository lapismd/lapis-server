<script lang="ts">
  import McpConsent from "../../web/src/account/McpConsent.svelte";
  import type { HostedVault } from "../../web/src/hosted/backend";

  const vaults: HostedVault[] = [
    { id: "notes", name: "Notes", role: "owner" },
    { id: "projects", name: "Projects", role: "owner" },
    { id: "archive", name: "Archive", role: "owner", archivedAt: 1 },
  ];

  let decision = $state("");
</script>

{#if decision}
  <section class="server-centered-surface">
    <p role="status">{decision}</p>
  </section>
{:else}
  <McpConsent
    requestId="storybook-mcp-request"
    username="owner"
    {vaults}
    transport={{
      load: async () => ({
        client_name: "Example AI client",
        client_id: "storybook-example-client",
        callback_host: "client.example",
        permissions: [
          "skills:read",
          "skills:write",
          "tasks:read",
          "tasks:write",
        ],
      }),
      decide: async (_path, result) => {
        decision = result.approve
          ? "Sample request approved."
          : "Sample request denied.";
      },
    }}
  />
{/if}
