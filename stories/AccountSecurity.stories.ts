import type { Meta, StoryObj } from "@storybook/svelte-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import AccountSecurityJourney from "./fixtures/AccountSecurityJourney.svelte";
import { disposeServerHostFixtures } from "./fixtures/ServerHostFixture.svelte";

const meta = {
  title: "Shell/Account & security",
  component: AccountSecurityJourney,
  tags: ["visual-pending"],
  beforeEach: () => disposeServerHostFixtures,
} satisfies Meta<typeof AccountSecurityJourney>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Accounts: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    async function openManageVaults() {
      await userEvent.click(
        await canvas.findByRole("button", { name: /Current workspace:/ }),
      );
      await userEvent.click(
        await within(canvasElement.ownerDocument.body).findByRole("menuitem", {
          name: /^Manage Vaults$/i,
        }),
      );
      return canvas.findByRole("heading", { name: "Your vaults" });
    }

    const vaultTitle = await openManageVaults();
    const createVault = canvas.getByRole("button", {
      name: /^Create New Vault/,
    });
    const vaultActionBackground = getComputedStyle(createVault).backgroundColor;
    const vaultActionForeground = getComputedStyle(createVault).color;
    const vaultTitleBounds = vaultTitle.getBoundingClientRect();
    const vaultGridBounds = canvasElement
      .querySelector(".server-vault-grid")!
      .getBoundingClientRect();
    const accountButton = await canvas.findByRole("button", {
      name: "Account & security",
    });
    await expect(accountButton.querySelector("svg")).toHaveClass(
      "lucide-shield-check",
    );
    await expect(getComputedStyle(accountButton).justifyContent).toBe("center");
    const accountButtonBounds = accountButton.getBoundingClientRect();
    await userEvent.click(accountButton);
    const vaultsButton = canvas.getByRole("button", { name: "Manage vaults" });
    await expect(vaultsButton).toBe(accountButton);
    const vaultsButtonBounds = vaultsButton.getBoundingClientRect();
    await expect(
      Math.abs(vaultsButtonBounds.left - accountButtonBounds.left),
    ).toBeLessThan(1);
    await expect(
      Math.abs(vaultsButtonBounds.top - accountButtonBounds.top),
    ).toBeLessThan(1);
    const securityTitleBounds = canvas
      .getByRole("heading", { name: "Account & security" })
      .getBoundingClientRect();
    const securityGridBounds = canvas
      .getByRole("complementary", { name: "Loaded accounts" })
      .getBoundingClientRect();
    await expect(
      Math.abs(securityTitleBounds.left - vaultTitleBounds.left),
    ).toBeLessThan(1);
    await expect(
      Math.abs(securityTitleBounds.top - vaultTitleBounds.top),
    ).toBeLessThan(1);
    await expect(
      Math.abs(securityGridBounds.left - vaultGridBounds.left),
    ).toBeLessThan(1);
    await expect(
      Math.abs(securityGridBounds.top - vaultGridBounds.top),
    ).toBeLessThan(1);
    const accountsPanel = canvas.getByRole("complementary", {
      name: "Loaded accounts",
    });
    const passkeysPanel = canvas.getByRole("region", {
      name: "Security settings for Owner",
    });
    const accountsHeader = accountsPanel.querySelector(
      ".security-card-header",
    )!;
    const passkeysHeader = passkeysPanel.querySelector(
      ".security-card-header",
    )!;
    const headingPadding = getComputedStyle(accountsHeader).paddingBlockEnd;
    await expect(Number.parseFloat(headingPadding)).toBeGreaterThan(0);
    await expect(getComputedStyle(passkeysHeader).paddingBlockEnd).toBe(
      headingPadding,
    );
    const sidebar = within(accountsPanel);
    const apiDocs = sidebar.getByRole("link", {
      name: "Server API documentation",
    });
    await expect(apiDocs).toHaveAttribute("href", "/api/docs");
    const docsStyle = getComputedStyle(apiDocs);
    const expectedColor =
      canvasElement.ownerDocument.createElement("div").style;
    expectedColor.backgroundColor = docsStyle.getPropertyValue("--border");
    await expect(docsStyle.backgroundColor).toBe(expectedColor.backgroundColor);
    await expect(vaultsButton.querySelector("svg")).toHaveClass(
      "lucide-database-cog",
    );
    await expect(getComputedStyle(vaultsButton).justifyContent).toBe("center");
    const owner = sidebar.getByRole("button", {
      name: "Owner Current account",
    });
    const example = sidebar.getByRole("button", {
      name: "Example account Preview account",
    });

    await expect(owner).toHaveAttribute("aria-pressed", "true");
    const ownerRow = getComputedStyle(owner.closest("li")!);
    const exampleRow = getComputedStyle(example.closest("li")!);
    await expect(ownerRow.backgroundColor).toBe(vaultActionBackground);
    await expect(getComputedStyle(owner).color).toBe(vaultActionForeground);
    await userEvent.click(example);
    await expect(exampleRow.backgroundColor).toBe(vaultActionBackground);
    await expect(ownerRow.backgroundColor).not.toBe(vaultActionBackground);
    const detail = within(
      canvas.getByRole("region", {
        name: "Security settings for Example account",
      }),
    );
    await expect(detail.getByText("No passkeys enrolled.")).toBeVisible();

    const menu = sidebar.getByRole("button", {
      name: "Open actions for Example account",
    });
    menu.focus();
    await userEvent.keyboard("{Enter}");
    const body = canvasElement.ownerDocument.body;
    const page = within(body);
    const waitForDialogClose = () =>
      waitFor(() => {
        expect(page.queryByRole("dialog")).not.toBeInTheDocument();
        expect(getComputedStyle(body).pointerEvents).not.toBe("none");
      });
    await userEvent.click(
      await page.findByRole("menuitem", { name: "Add passkey…" }),
    );
    const dialog = within(
      await page.findByRole("dialog", { name: "Add a passkey" }),
    );
    await expect(
      dialog.getByRole("button", { name: "Add passkey" }),
    ).toBeDisabled();
    await userEvent.type(dialog.getByLabelText("Passkey name"), "Test device");
    await userEvent.click(dialog.getByRole("button", { name: "Add passkey" }));
    await waitForDialogClose();
    await expect(detail.getByText("Test device")).toBeVisible();

    await userEvent.click(
      detail.getByRole("button", { name: "Open actions for Test device" }),
    );
    await userEvent.click(
      await page.findByRole("menuitem", { name: "Rename…" }),
    );
    const rename = within(
      await page.findByRole("dialog", { name: "Rename passkey" }),
    );
    await userEvent.clear(rename.getByLabelText("Passkey name"));
    await userEvent.type(rename.getByLabelText("Passkey name"), "New name");
    await userEvent.click(rename.getByRole("button", { name: "Save name" }));
    await waitForDialogClose();
    await expect(detail.getByText("New name")).toBeVisible();

    await userEvent.click(
      detail.getByRole("button", { name: "Open actions for New name" }),
    );
    await userEvent.click(
      await page.findByRole("menuitem", { name: "Remove…" }),
    );
    const remove = within(
      await page.findByRole("dialog", { name: "Remove passkey" }),
    );
    await userEvent.click(
      remove.getByRole("button", { name: "Remove passkey" }),
    );
    await waitForDialogClose();
    await expect(detail.getByText("No passkeys enrolled.")).toBeVisible();

    await userEvent.click(owner);
    await expect(canvas.getByText("MacBook Pro")).toBeVisible();
    const bounds = canvas.getByRole("main").getBoundingClientRect();
    const sidebarBounds = canvas
      .getByRole("complementary")
      .getBoundingClientRect();
    const detailBounds = canvas
      .getByRole("region", { name: "Security settings for Owner" })
      .getBoundingClientRect();
    if (bounds.width > 832) {
      await expect(sidebarBounds.right).toBeLessThan(detailBounds.left);
    } else {
      await expect(sidebarBounds.bottom).toBeLessThan(detailBounds.top);
    }
    const ownerPanel = within(
      canvas.getByRole("region", { name: "Security settings for Owner" }),
    );
    await expect(
      ownerPanel.getByRole("heading", { name: "Passkeys" }),
    ).toBeVisible();
    await expect(
      ownerPanel.queryByText("Current account"),
    ).not.toBeInTheDocument();
    await userEvent.click(
      canvas.getByRole("button", { name: "Manage vaults" }),
    );
    await expect(
      await canvas.findByRole("heading", { name: "Your vaults" }),
    ).toBeVisible();
    await waitFor(() => expect(accountButton).toHaveFocus());

    await userEvent.click(accountButton);
    await userEvent.click(
      canvas.getByRole("button", { name: "Return to Notes" }),
    );
    await expect(
      await canvas.findByRole("button", { name: /Current workspace:/ }),
    ).toBeVisible();
    await openManageVaults();
  },
};

export const AccountsDark: Story = {
  ...Accounts,
  globals: { colorMode: "dark" },
};
