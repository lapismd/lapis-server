<script lang="ts">
  import BookOpen from "@lucide/svelte/icons/book-open";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import KeyRound from "@lucide/svelte/icons/key-round";
  import PenLine from "@lucide/svelte/icons/pen-line";
  import Plus from "@lucide/svelte/icons/plus";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import UserRound from "@lucide/svelte/icons/user-round";
  import * as Button from "@lapismd/design-core/shadcn/button";
  import * as Card from "@lapismd/design-core/shadcn/card";
  import * as Dialog from "@lapismd/design-core/shadcn/dialog";
  import * as DropdownMenu from "@lapismd/design-core/shadcn/dropdown-menu";
  import * as Input from "@lapismd/design-core/shadcn/input";

  type Passkey = { id: string; label: string; activity: string };
  type Account = {
    id: string;
    label: string;
    description: string;
    passkeys: Passkey[];
  };
  type Action =
    | { kind: "add" | "verify" }
    | { kind: "rename" | "remove"; key: Passkey };

  let accounts = $state<Account[]>([
    {
      id: "owner",
      label: "Owner",
      description: "Current account",
      passkeys: [
        { id: "laptop", label: "MacBook Pro", activity: "Last used today" },
        { id: "key", label: "Security key", activity: "Unused" },
      ],
    },
    {
      id: "example",
      label: "Example account",
      description: "Preview account",
      passkeys: [],
    },
  ]);
  let selectedId = $state("owner");
  let account = $derived(accounts.find((entry) => entry.id === selectedId)!);
  let action = $state<Action>({ kind: "add" });
  let dialogOpen = $state(false);
  let name = $state("");
  let notice = $state("");

  const titles = {
    add: "Add a passkey",
    verify: "Verify identity",
    rename: "Rename passkey",
    remove: "Remove passkey",
  };

  function select(id: string) {
    selectedId = id;
    notice = "";
  }

  function openAction(id: string, next: Action) {
    select(id);
    action = next;
    name = next.kind === "rename" ? next.key.label : "";
    dialogOpen = true;
  }

  function confirm(event: SubmitEvent) {
    event.preventDefault();
    switch (action.kind) {
      case "add":
        account.passkeys.push({
          id: crypto.randomUUID(),
          label: name.trim(),
          activity: "Unused",
        });
        notice = "Passkey added.";
        break;
      case "rename":
        action.key.label = name.trim();
        notice = "Passkey renamed.";
        break;
      case "remove": {
        const id = action.key.id;
        account.passkeys = account.passkeys.filter((key) => key.id !== id);
        notice = "Passkey removed.";
        break;
      }
      case "verify":
        notice = "Identity verified.";
        break;
    }
    dialogOpen = false;
  }
</script>

<section
  class="security-preview"
  aria-labelledby="server-account-security-heading"
>
  <div class="server-vault-grid security-grid">
    <aside aria-label="Loaded accounts">
      <Card.Root class="server-vault-card">
        <Card.Header class="security-card-header">
          <Card.Title class="server-card-title">Accounts</Card.Title>
        </Card.Header>
        <Card.Content class="server-managed-content">
          <ul class="server-vault-list">
            {#each accounts as entry (entry.id)}
              <li
                class="server-vault-row"
                class:selected={entry.id === selectedId}
              >
                <button
                  type="button"
                  class="server-vault-open security-account"
                  aria-pressed={entry.id === selectedId}
                  onclick={() => select(entry.id)}
                >
                  <UserRound aria-hidden="true" />
                  <span class="server-vault-copy">
                    <span class="server-vault-name">{entry.label}</span>
                    <span class="server-vault-meta">{entry.description}</span>
                  </span>
                </button>
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger
                    class="server-vault-menu-trigger"
                    aria-label={`Open actions for ${entry.label}`}
                  >
                    <Ellipsis aria-hidden="true" />
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Content align="end">
                    <DropdownMenu.Group>
                      <DropdownMenu.Item
                        onclick={() => openAction(entry.id, { kind: "add" })}
                      >
                        <Plus aria-hidden="true" />
                        Add passkey…
                      </DropdownMenu.Item>
                      <DropdownMenu.Item
                        onclick={() => openAction(entry.id, { kind: "verify" })}
                      >
                        <ShieldCheck aria-hidden="true" />
                        Verify identity…
                      </DropdownMenu.Item>
                    </DropdownMenu.Group>
                  </DropdownMenu.Content>
                </DropdownMenu.Root>
              </li>
            {/each}
          </ul>
          <a
            class="api-docs-link web-vault-launcher__recent"
            href="/api/docs"
            target="_blank"
            rel="noopener"
          >
            <BookOpen aria-hidden="true" />
            Server API documentation
          </a>
        </Card.Content>
      </Card.Root>
    </aside>

    <section aria-label={`Security settings for ${account.label}`}>
      <Card.Root class="server-vault-card">
        <Card.Header class="security-card-header">
          <h2 class="server-card-title passkeys-heading">Passkeys</h2>
        </Card.Header>
        <Card.Content class="server-managed-content passkeys">
          <ul class="server-vault-list">
            {#each account.passkeys as key (key.id)}
              <li class="server-vault-row">
                <div class="server-vault-open passkey-summary">
                  <KeyRound aria-hidden="true" />
                  <span class="server-vault-copy">
                    <span class="server-vault-name">{key.label}</span>
                    <span class="server-vault-meta">{key.activity}</span>
                  </span>
                </div>
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger
                    class="server-vault-menu-trigger"
                    aria-label={`Open actions for ${key.label}`}
                  >
                    <Ellipsis aria-hidden="true" />
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Content align="end">
                    <DropdownMenu.Group>
                      <DropdownMenu.Item
                        onclick={() =>
                          openAction(account.id, { kind: "rename", key })}
                      >
                        <PenLine aria-hidden="true" />
                        Rename…
                      </DropdownMenu.Item>
                    </DropdownMenu.Group>
                    <DropdownMenu.Separator />
                    <DropdownMenu.Group>
                      <DropdownMenu.Item
                        variant="destructive"
                        onclick={() =>
                          openAction(account.id, { kind: "remove", key })}
                      >
                        <Trash2 aria-hidden="true" />
                        Remove…
                      </DropdownMenu.Item>
                    </DropdownMenu.Group>
                  </DropdownMenu.Content>
                </DropdownMenu.Root>
              </li>
            {:else}
              <li class="server-vault-empty">No passkeys enrolled.</li>
            {/each}
          </ul>
          {#if notice}<p class="server-alert" role="status">{notice}</p>{/if}
        </Card.Content>
      </Card.Root>
    </section>
  </div>
</section>

<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>{titles[action.kind]}</Dialog.Title>
      <Dialog.Description>
        {#if action.kind === "remove"}
          Remove {action.key.label} from {account.label}?
        {:else}
          {account.label}
        {/if}
      </Dialog.Description>
    </Dialog.Header>
    <form onsubmit={confirm}>
      {#if action.kind === "add" || action.kind === "rename"}
        <label for="preview-passkey-name">Passkey name</label>
        <Input.Root
          id="preview-passkey-name"
          bind:value={name}
          maxlength={100}
          required
        />
      {/if}
      <Dialog.Footer>
        <Button.Root
          type="button"
          variant="outline"
          onclick={() => (dialogOpen = false)}>Cancel</Button.Root
        >
        <Button.Root
          type="submit"
          variant={action.kind === "remove" ? "destructive" : "default"}
          disabled={(action.kind === "add" || action.kind === "rename") &&
            !name.trim()}
        >
          {action.kind === "add"
            ? "Add passkey"
            : action.kind === "rename"
              ? "Save name"
              : action.kind === "remove"
                ? "Remove passkey"
                : "Verify"}
        </Button.Root>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<style>
  .security-preview {
    position: absolute;
    inset: 0 0 auto;
  }

  .security-grid {
    grid-template-columns: minmax(0, 19rem) minmax(0, 1fr);
    align-items: start;
  }

  .security-grid > * {
    min-width: 0;
  }

  .security-account,
  .passkey-summary {
    min-width: 0;
    flex: 1;
    gap: 0.65rem;
    padding: 0.65rem;
  }

  .security-account {
    background: transparent;
    border: 0;
    border-radius: 0.5rem;
    color: inherit;
    cursor: pointer;
  }

  .security-grid :global(.security-card-header) {
    padding-inline: 1rem;
  }

  .security-grid li.server-vault-row {
    background: var(--background);
  }

  .security-grid li.selected,
  .security-grid li.server-vault-row:hover {
    background: var(--accent);
    color: var(--accent-foreground);
  }

  .security-account:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .security-grid :global(.passkeys),
  form {
    display: grid;
    gap: 0.75rem;
  }

  .passkeys-heading {
    font-weight: 600;
  }

  .api-docs-link {
    margin-top: 0.5rem;
    color: var(--foreground);
    font-size: 0.875rem;
  }

  .api-docs-link :global(svg) {
    width: 1rem;
    height: 1rem;
    flex: none;
  }

  @media (max-width: 52rem) {
    .security-grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
