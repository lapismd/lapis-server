<script lang="ts">
  import { onMount } from "svelte";
  import * as Button from "@lapismd/design-core/shadcn/button";
  import * as Card from "@lapismd/design-core/shadcn/card";
  import * as Input from "@lapismd/design-core/shadcn/input";
  import { RustHostError, RustHttpClient } from "./hosted/rust/client";

  type Connection = {
    id: string;
    client_id: string;
    client_name: string;
    expires_at: number;
    status: "pending" | "approved" | "rejected" | "consumed" | "expired";
  };
  let {
    requestId,
    host = new RustHttpClient(),
  }: {
    requestId: string;
    host?: RustHttpClient;
  } = $props();
  let viewState = $state<"loading" | "login" | "review" | "settled" | "failed">(
    "loading",
  );
  let connection = $state<Connection | null>(null);
  let username = $state("");
  let password = $state("");
  let owner = $state("");
  let message = $state("");
  let busy = $state(false);
  let disposed = false;
  const controller = new AbortController();
  const validId =
    /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;

  onMount(() => {
    void initialize();
    return () => {
      disposed = true;
      controller.abort();
    };
  });

  async function inspect(): Promise<void> {
    const current = await host.current();
    if (disposed) return;
    const result = await host.json<Connection>(
      `/sync/account/${requestId}`,
      "GET",
      undefined,
      controller.signal,
    );
    if (disposed) return;
    if (
      result.id !== requestId ||
      !validId.test(result.client_id) ||
      typeof result.client_name !== "string" ||
      result.client_name.length > 128 ||
      !Number.isSafeInteger(result.expires_at) ||
      !["pending", "approved", "rejected", "consumed", "expired"].includes(
        result.status,
      )
    ) {
      throw new Error("The connection request could not be verified.");
    }
    owner = current.username;
    connection = result;
    viewState = result.status === "pending" ? "review" : "settled";
  }

  async function initialize(): Promise<void> {
    if (!validId.test(requestId)) {
      viewState = "failed";
      message = "This connection link is invalid.";
      return;
    }
    try {
      const info = await host.info();
      if (disposed) return;
      if (info.protocol_major !== host.protocolMajor || !info.browser_auth) {
        throw new Error("This server does not support account consent.");
      }
      await inspect();
    } catch (error) {
      if (disposed) return;
      if (error instanceof RustHostError && error.status === 401)
        viewState = "login";
      else {
        viewState = "failed";
        message =
          "The connection request is unavailable. Open a fresh link from your device.";
      }
    }
  }

  async function login(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (busy) return;
    busy = true;
    message = "";
    try {
      await host.login(username, password);
      password = "";
      if (!disposed) await inspect();
    } catch {
      if (!disposed)
        message =
          "Sign-in failed or the request is unavailable. Check your details and try again.";
    } finally {
      password = "";
      busy = false;
    }
  }

  async function decide(approve: boolean): Promise<void> {
    if (busy || viewState !== "review" || !connection) return;
    busy = true;
    try {
      await host.json(
        `/sync/account/${requestId}/decision`,
        "POST",
        { approve },
        controller.signal,
      );
      if (disposed) return;
      connection = { ...connection, status: approve ? "approved" : "rejected" };
      viewState = "settled";
    } catch {
      if (disposed) return;
      viewState = "failed";
      message =
        "The decision could not be confirmed. Reload this page to inspect its status before trying again.";
    } finally {
      busy = false;
    }
  }
</script>

<main class="server-centered-surface server-login" data-sync-account-consent>
  <Card.Root class="server-login-card">
    <Card.Header>
      <h1 class="server-login-title">Connect to Lapis Server</h1>
      <Card.Description
        >Review a device’s request to connect to your account.</Card.Description
      >
    </Card.Header>
    <Card.Content class="server-login-fields">
      {#if viewState === "loading"}
        <p role="status">Loading connection request…</p>
      {:else if viewState === "login"}
        <form onsubmit={login} class="server-login-fields">
          <h2>Sign in to review</h2>
          <label for="sync-owner">Username</label>
          <Input.Root
            id="sync-owner"
            autocomplete="username"
            bind:value={username}
            required
            disabled={busy}
          />
          <label for="sync-password">Password</label>
          <Input.Root
            id="sync-password"
            type="password"
            autocomplete="current-password"
            bind:value={password}
            required
            disabled={busy}
          />
          <Button.Root type="submit" disabled={busy}
            >{busy ? "Signing in…" : "Sign in"}</Button.Root
          >
        </form>
      {:else if viewState === "review" && connection}
        <h2>{connection.client_name}</h2>
        <p>
          Signed in as {owner}. Approve only if you started this request on a
          device you trust.
        </p>
        <p>
          Device ID: <span class="sync-client-id">{connection.client_id}</span>
        </p>
        <p>
          This connection can discover your managed vault names and identities.
          Access to notes, attachments and encryption keys requires separate
          enrollment.
        </p>
        <Button.Root
          onclick={() => decide(false)}
          variant="outline"
          disabled={busy}>Reject</Button.Root
        >
        <Button.Root onclick={() => decide(true)} disabled={busy}
          >{busy ? "Saving decision…" : "Approve connection"}</Button.Root
        >
      {:else if viewState === "settled" && connection}
        <p role="status">
          {connection.status === "approved" || connection.status === "consumed"
            ? "Connection approved. Return to your device to continue."
            : connection.status === "rejected"
              ? "Connection rejected. You can close this page."
              : "This connection request has expired. Start a new request on your device."}
        </p>
      {/if}
      {#if message}<p role="alert">{message}</p>{/if}
    </Card.Content>
  </Card.Root>
</main>

<style>
  .sync-client-id {
    overflow-wrap: anywhere;
  }
</style>
