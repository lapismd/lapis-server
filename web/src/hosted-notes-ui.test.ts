import { expect, test } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/svelte";
import ServerVaultHost from "./ServerVaultHost.svelte";
import serverLogo from "./assets/lapis-logo.svg?url";
import { createHostedFixture } from "../../stories/fixtures/hosted-backend";
import { afterEach } from "vitest";

afterEach(cleanup);

test("production login retains its standalone branded form", async () => {
  const { backend } = createHostedFixture("login");
  const view = render(ServerVaultHost, { backend });
  await view.findByRole("heading", { name: "Sign in" });
  expect(view.getByLabelText("Username")).toBeTruthy();
  expect(view.getByLabelText("Password")).toBeTruthy();
  expect(
    view.container
      .querySelector("[data-server-login-logo]")
      ?.getAttribute("src"),
  ).toBe(serverLogo);
  expect(
    view.container.querySelector('[data-ui-component="lapis-workspace-shell"]'),
  ).toBeNull();
});

test("production login reports rejection and clears the password before retry", async () => {
  const { backend } = createHostedFixture("login");
  const view = render(ServerVaultHost, { backend });
  await view.findByLabelText("Username");
  await fireEvent.input(view.getByLabelText("Username"), {
    target: { value: "owner" },
  });
  await fireEvent.input(view.getByLabelText("Password"), {
    target: { value: "incorrect" },
  });
  await fireEvent.submit(view.getByLabelText("Password").closest("form")!);
  expect((await view.findByRole("alert")).textContent).toContain(
    "Invalid username or password",
  );
  expect((view.getByLabelText("Password") as HTMLInputElement).value).toBe("");
});

test("production login enters the managed vault chooser", async () => {
  const { backend } = createHostedFixture("login");
  const view = render(ServerVaultHost, { backend });
  await view.findByLabelText("Username");
  await fireEvent.input(view.getByLabelText("Username"), {
    target: { value: "owner" },
  });
  await fireEvent.input(view.getByLabelText("Password"), {
    target: { value: "fixture-password" },
  });
  await fireEvent.submit(view.getByLabelText("Password").closest("form")!);
  await view.findByRole("heading", { name: "Your vaults" });
  expect(
    view.getByRole("button", { name: "Open Notes owner · server-hosted" }),
  ).toBeTruthy();
});
