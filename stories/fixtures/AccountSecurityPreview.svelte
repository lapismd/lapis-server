<script lang="ts">
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
  import serverLogo from "../../web/src/assets/lapis-logo.svg?url";

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

<main class="server-vault-launcher security-preview">
  <div class="security-content">
    <header class="server-launcher-header">
      <div class="server-brand-heading">
        <img src={serverLogo} alt="" class="server-launcher-logo" />
        <div class="server-brand-copy">
          <p class="server-eyebrow">Lapis Notes</p>
          <h1 class="server-launcher-title">Account &amp; security</h1>
          <p class="server-launcher-description">
            Manage how you sign in to your server account.
          </p>
        </div>
      </div>
    </header>

    <div class="server-vault-grid security-grid">
      <aside aria-label="Loaded accounts">
        <Card.Root class="server-vault-card">
          <Card.Header>
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
                          onclick={() =>
                            openAction(entry.id, { kind: "verify" })}
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
          </Card.Content>
        </Card.Root>
      </aside>

      <section aria-label={`Security settings for ${account.label}`}>
        <Card.Root class="server-vault-card">
          <Card.Header>
            <Card.Title class="server-card-title">{account.label}</Card.Title>
            <Card.Description>{account.description}</Card.Description>
          </Card.Header>
          <Card.Content class="server-managed-content passkeys">
            <h2 class="passkeys-heading">Passkeys</h2>
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
  </div>
</main>

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
    height: 100dvh;
    overflow: auto;
  }

  .security-content {
    display: grid;
    gap: 1.25rem;
    max-width: 72rem;
    margin-inline: auto;
    padding: clamp(1rem, 4vw, 2.5rem);
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

  .security-account:hover,
  .selected {
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
    font-size: 0.875rem;
    font-weight: 600;
  }

  @media (max-width: 52rem) {
    .security-grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
