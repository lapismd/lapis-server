<script lang="ts" module>
  import type { AccountSecurity } from "./passkeys";

  export type SecurityAccount = {
    id: string;
    label: string;
    description: string;
    security?: Omit<AccountSecurity, "signIn">;
  };
</script>

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

  import { onMount } from "svelte";
  import { securityError, type PasskeySummary } from "./passkeys";

  type Action =
    | { kind: "add" | "verify" }
    | { kind: "rename" | "remove"; key: PasskeySummary };

  let {
    accounts,
    canUsePasskeys,
  }: {
    accounts: readonly [SecurityAccount, ...SecurityAccount[]];
    canUsePasskeys: boolean;
  } = $props();
  const id = $props.id();
  let selectedId = $state<string>();
  let account = $derived(
    accounts.find((entry) => entry.id === selectedId) ?? accounts[0],
  );
  let keys = $state<PasskeySummary[]>([]);
  let action = $state<Action>({ kind: "add" });
  let dialogOpen = $state(false);
  let name = $state("");
  let password = $state("");
  let notice = $state("");
  let loadError = $state("");
  let actionError = $state("");
  let loading = $state(false);
  let busy = $state(false);
  let loadVersion = 0;

  const titles = {
    add: "Add a passkey",
    verify: "Verify identity",
    rename: "Rename passkey",
    remove: "Remove passkey",
  };

  onMount(() => {
    void load(account);
    return () => {
      loadVersion += 1;
    };
  });

  async function load(entry: SecurityAccount) {
    if (!entry.security) return;
    const version = ++loadVersion;
    loadError = "";
    loading = true;
    try {
      const result = await entry.security.passkeys();
      if (version === loadVersion) keys = result;
    } catch (error) {
      if (version === loadVersion) loadError = securityError(error);
    } finally {
      if (version === loadVersion) loading = false;
    }
  }

  function select(entry: SecurityAccount) {
    if (entry.id === account.id) return;
    selectedId = entry.id;
    keys = [];
    notice = "";
    loadError = "";
    loading = false;
    loadVersion += 1;
    void load(entry);
  }

  function openAction(entry: SecurityAccount, next: Action) {
    select(entry);
    action = next;
    name = next.kind === "rename" ? next.key.label : "";
    password = "";
    actionError = "";
    notice = "";
    dialogOpen = true;
  }

  async function submit(verifyWithPasskey = false) {
    const entry = account;
    const security = entry.security;
    if (busy || !security) return;
    const current = action;
    const secret = password;
    password = "";
    busy = true;
    actionError = "";
    try {
      switch (current.kind) {
        case "add":
          await security.enroll(name.trim());
          notice = "Passkey added.";
          break;
        case "rename":
          await security.rename(current.key.id, name.trim());
          notice = "Passkey renamed.";
          break;
        case "remove":
          await security.remove(current.key.id);
          notice = "Passkey removed.";
          break;
        case "verify":
          if (verifyWithPasskey) await security.verifyPasskey();
          else await security.verifyPassword(secret);
          notice = "Identity verified.";
          break;
      }
      dialogOpen = false;
      if (current.kind !== "verify") await load(entry);
    } catch (error) {
      actionError = securityError(error);
    } finally {
      busy = false;
    }
  }
</script>

<section
  class="security-page"
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
                class:selected={entry.id === account.id}
              >
                <button
                  type="button"
                  class="server-vault-open security-account"
                  aria-pressed={entry.id === account.id}
                  onclick={() => select(entry)}
                >
                  <UserRound aria-hidden="true" />
                  <span class="server-vault-copy">
                    <span class="server-vault-name">{entry.label}</span>
                    <span class="server-vault-meta">{entry.description}</span>
                  </span>
                </button>
                {#if entry.security}
                  <DropdownMenu.Root>
                    <DropdownMenu.Trigger
                      class="server-vault-menu-trigger"
                      aria-label={`Open actions for ${entry.label}`}
                    >
                      <Ellipsis aria-hidden="true" />
                    </DropdownMenu.Trigger>
                    <DropdownMenu.Content align="end">
                      <DropdownMenu.Group>
                        {#if canUsePasskeys}
                          <DropdownMenu.Item
                            onclick={() => openAction(entry, { kind: "add" })}
                          >
                            <Plus aria-hidden="true" />
                            Add passkey…
                          </DropdownMenu.Item>
                        {/if}
                        <DropdownMenu.Item
                          onclick={() => openAction(entry, { kind: "verify" })}
                        >
                          <ShieldCheck aria-hidden="true" />
                          Verify identity…
                        </DropdownMenu.Item>
                      </DropdownMenu.Group>
                    </DropdownMenu.Content>
                  </DropdownMenu.Root>
                {/if}
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
          {#if !account.security}
            <p>Passkeys are unavailable on this server.</p>
          {:else if loadError}
            <p class="server-alert server-alert--error" role="alert">
              {loadError}
            </p>
            <Button.Root
              type="button"
              variant="outline"
              onclick={() => void load(account)}>Retry</Button.Root
            >
          {:else if loading}
            <p role="status">Loading passkeys…</p>
          {:else}
            <ul class="server-vault-list">
              {#each keys as key (key.id)}
                <li class="server-vault-row">
                  <div class="server-vault-open passkey-summary">
                    <KeyRound aria-hidden="true" />
                    <span class="server-vault-copy">
                      <span class="server-vault-name">{key.label}</span>
                      <span class="server-vault-meta">
                        {key.last_used_at
                          ? `Last used ${new Date(key.last_used_at).toLocaleString()}`
                          : "Unused"}
                      </span>
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
                            openAction(account, { kind: "rename", key })}
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
                            openAction(account, { kind: "remove", key })}
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
          {/if}
          {#if notice}<p class="server-alert" role="status">{notice}</p>{/if}
        </Card.Content>
      </Card.Root>
    </section>
  </div>
</section>

<Dialog.Root
  bind:open={
    () => dialogOpen,
    (open) => {
      if (!busy) dialogOpen = open;
    }
  }
>
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
    <form
      onsubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
      aria-busy={busy}
    >
      {#if action.kind === "add" || action.kind === "rename"}
        <label for={`${id}-passkey-name`}>Passkey name</label>
        <Input.Root
          id={`${id}-passkey-name`}
          bind:value={name}
          disabled={busy}
          maxlength={100}
          required
        />
      {:else if action.kind === "verify"}
        <label for={`${id}-password`}>Verify identity with password</label>
        <Input.Root
          id={`${id}-password`}
          bind:value={password}
          type="password"
          autocomplete="current-password"
          disabled={busy}
          required
        />
      {/if}
      {#if actionError}<p class="server-alert server-alert--error" role="alert">
          {actionError}
        </p>{/if}
      <Dialog.Footer>
        {#if action.kind === "verify" && canUsePasskeys && keys.length}
          <Button.Root
            type="button"
            variant="outline"
            disabled={busy}
            onclick={() => void submit(true)}
          >
            Verify with a passkey
          </Button.Root>
        {/if}
        <Button.Root
          type="button"
          variant="outline"
          disabled={busy}
          onclick={() => (dialogOpen = false)}>Cancel</Button.Root
        >
        <Button.Root
          type="submit"
          variant={action.kind === "remove" ? "destructive" : "default"}
          disabled={busy ||
            ((action.kind === "add" || action.kind === "rename") &&
              !name.trim()) ||
            (action.kind === "verify" && !password)}
        >
          {action.kind === "add"
            ? "Add passkey"
            : action.kind === "rename"
              ? "Save name"
              : action.kind === "remove"
                ? "Remove passkey"
                : "Verify password"}
        </Button.Root>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<style>
  .security-page {
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
    padding-block-end: 0.5rem;
  }

  .security-grid li.server-vault-row {
    background: var(--background);
  }

  .security-grid li.server-vault-row:hover:not(.selected) {
    background: var(--accent);
    color: var(--accent-foreground);
  }

  .security-grid li.selected {
    background: var(--ui-button-background, var(--primary));
    color: var(--ui-button-foreground, var(--primary-foreground));
  }

  .selected .server-vault-meta,
  .selected :global(.server-vault-menu-trigger) {
    color: inherit;
  }

  .selected :global(.server-vault-menu-trigger:hover:not(:disabled)) {
    background: color-mix(in srgb, currentColor 16%, transparent);
  }

  .security-account:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .selected .security-account:focus-visible,
  .selected :global(.server-vault-menu-trigger:focus-visible) {
    outline-color: currentColor;
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
    background: var(--border);
    font-size: 0.875rem;
  }

  .api-docs-link.web-vault-launcher__recent:hover {
    background: color-mix(in srgb, var(--foreground) 8%, var(--border));
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
