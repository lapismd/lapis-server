import type { Meta, StoryObj } from "@storybook/svelte-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import AccountSecurityPreview from "./fixtures/AccountSecurityPreview.svelte";

const meta = {
  title: "Shell/Account & security",
  component: AccountSecurityPreview,
  tags: ["visual-pending"],
} satisfies Meta<typeof AccountSecurityPreview>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Accounts: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const sidebar = within(
      canvas.getByRole("complementary", { name: "Loaded accounts" }),
    );
    const owner = sidebar.getByRole("button", {
      name: "Owner Current account",
    });
    const example = sidebar.getByRole("button", {
      name: "Example account Preview account",
    });

    await expect(owner).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(example);
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
  },
};

export const AccountsDark: Story = {
  ...Accounts,
  globals: { colorMode: "dark" },
};
