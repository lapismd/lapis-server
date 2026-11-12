import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render } from "@testing-library/svelte";
import { tick } from "svelte";
import ServerVaultHost from "./ServerVaultHost.svelte";
import {
  createHostedFixture,
  createFixtureVaultRestoration,
} from "../../stories/fixtures/hosted-backend";

afterEach(cleanup);

test.each(["missing", "archive"])(
  "unavailable remembered vault %s stays in the chooser without admission",
  async (vaultId) => {
    const { backend } = createHostedFixture("vaults");
    const prepare = vi.spyOn(backend, "prepare");
    const restoration = createFixtureVaultRestoration();
    restoration.remember("owner", vaultId);
    const view = render(ServerVaultHost, { backend, restoration });
    await view.findByRole("heading", { name: "Your vaults" });
    expect((await view.findByRole("alert")).textContent).toContain(
      "previously selected vault is unavailable",
    );
    expect(prepare).not.toHaveBeenCalled();
    expect(restoration.take("owner")).toBeNull();
  },
);

test("a different authenticated owner cannot consume another owner's remembered vault", async () => {
  const { backend } = createHostedFixture("vaults");
  const prepare = vi.spyOn(backend, "prepare");
  const restoration = createFixtureVaultRestoration();
  restoration.remember("another owner", "notes");
  const view = render(ServerVaultHost, { backend, restoration });
  await view.findByRole("heading", { name: "Your vaults" });
  expect(prepare).not.toHaveBeenCalled();
  expect(restoration.take("another owner")).toBe("notes");
});

test("failed restoration remains recoverable and does not repeat on remount", async () => {
  const { backend } = createHostedFixture("vaults");
  const prepare = vi
    .spyOn(backend, "prepare")
    .mockRejectedValue(new Error("Vault unavailable"));
  const restoration = createFixtureVaultRestoration();
  restoration.remember("owner", "notes");
  const first = render(ServerVaultHost, { backend, restoration });
  expect((await first.findByRole("alert")).textContent).toContain(
    "Vault unavailable",
  );
  first.unmount();
  const second = render(ServerVaultHost, { backend, restoration });
  await second.findByRole("heading", { name: "Your vaults" });
  expect(prepare).toHaveBeenCalledTimes(1);
});

test("retired hosts do not admit an automatic restoration after authentication resolves", async () => {
  const { backend } = createHostedFixture("vaults");
  let authenticate!: (identity: { username: string }) => void;
  const identity = new Promise<{ username: string }>((resolve) => {
    authenticate = resolve;
  });
  const current = vi.spyOn(backend, "current").mockReturnValue(identity);
  const prepare = vi
    .spyOn(backend, "prepare")
    .mockRejectedValue(new Error("Unexpected admission"));
  const restoration = createFixtureVaultRestoration();
  restoration.remember("owner", "notes");
  const view = render(ServerVaultHost, { backend, restoration });
  await vi.waitFor(() => expect(current).toHaveBeenCalled());
  view.unmount();
  authenticate({ username: "owner" });
  await identity;
  await tick();
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(prepare).not.toHaveBeenCalled();
  expect(restoration.take("owner")).toBe("notes");
});
