<script lang="ts">
  import { onMount } from "svelte";
  import * as Button from "@lapismd/design-core/shadcn/button";
  import * as Input from "@lapismd/design-core/shadcn/input";
  import { securityError, type AccountSecurity } from "./passkeys";

  let { security }: { security: AccountSecurity } = $props();
  let keys = $state<Awaited<ReturnType<AccountSecurity["passkeys"]>>>([]);
  let labels = $state<Record<string, string>>({});
  let label = $state("");
  let password = $state("");
  let error = $state("");
  let notice = $state("");
  let busy = $state(false);

  async function load() {
    keys = await security.passkeys();
    labels = Object.fromEntries(keys.map((key) => [key.id, key.label]));
  }

  async function run(action: () => Promise<void>, done = "") {
    if (busy) return;
    busy = true;
    error = "";
    notice = "";
    try {
      await action();
      notice = done;
    } catch (failure) {
      error = securityError(failure);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void run(load);
  });
</script>

<div class="security" aria-busy={busy}>
  <section aria-label="Passkeys">
    {#if keys.length === 0}
      <p>No passkeys enrolled.</p>
    {/if}
    {#each keys as key (key.id)}
      <div class="key">
        <label>
          <span>Passkey name</span>
          <Input.Root bind:value={labels[key.id]} maxlength={100} />
        </label>
        <p>
          Added {new Date(key.created_at).toLocaleDateString()}.
          {key.last_used_at
            ? `Last used ${new Date(key.last_used_at).toLocaleString()}.`
            : "Unused."}
        </p>
        <div class="actions">
          <Button.Root
            type="button"
            variant="outline"
            disabled={busy ||
              !labels[key.id]?.trim() ||
              labels[key.id] === key.label}
            onclick={() =>
              void run(async () => {
                await security.rename(key.id, labels[key.id]);
                await load();
              }, "Passkey renamed.")}
          >
            Rename {key.label}
          </Button.Root>
          <Button.Root
            type="button"
            variant="destructive"
            disabled={busy}
            onclick={() =>
              void run(async () => {
                await security.remove(key.id);
                await load();
              }, "Passkey removed.")}
          >
            Remove {key.label}
          </Button.Root>
        </div>
      </div>
    {/each}
    <form
      onsubmit={(event) => {
        event.preventDefault();
        void run(async () => {
          await security.enroll(label);
          label = "";
          await load();
        }, "Passkey added.");
      }}
    >
      <label>
        <span>New passkey name</span>
        <Input.Root bind:value={label} maxlength={100} required />
      </label>
      <Button.Root type="submit" disabled={busy || !label.trim()}
        >Add passkey</Button.Root
      >
    </form>
  </section>

  <form
    onsubmit={(event) => {
      event.preventDefault();
      const secret = password;
      password = "";
      void run(() => security.verifyPassword(secret), "Identity verified.");
    }}
  >
    <label>
      <span>Verify identity with password</span>
      <Input.Root
        bind:value={password}
        type="password"
        autocomplete="current-password"
        required
      />
    </label>
    <div class="actions">
      <Button.Root type="submit" variant="outline" disabled={busy || !password}
        >Verify password</Button.Root
      >
      {#if keys.length}
        <Button.Root
          type="button"
          variant="outline"
          disabled={busy}
          onclick={() =>
            void run(() => security.verifyPasskey(), "Identity verified.")}
        >
          Verify with a passkey
        </Button.Root>
      {/if}
    </div>
  </form>
  {#if error}<p role="alert">{error}</p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}
</div>

<style>
  .security,
  section,
  .key,
  form,
  label {
    display: grid;
    gap: 0.6rem;
  }
  .security {
    max-height: 65vh;
    overflow: auto;
    padding: 0.15rem;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
</style>
