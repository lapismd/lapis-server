import type { Meta, StoryObj } from "@storybook/svelte-vite";
import { expect, userEvent, within } from "storybook/test";
import McpConsentFixture from "./fixtures/McpConsentFixture.svelte";

const meta = {
  title: "Shell/MCP authorization",
  component: McpConsentFixture,
  parameters: { layout: "fullscreen" },
  tags: ["visual-pending"],
} satisfies Meta<typeof McpConsentFixture>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Consent: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const search = await canvas.findByRole("combobox", {
      name: "Vaults",
    });
    const vaults = within(canvas.getByRole("listbox", { name: "Vaults" }));
    const permissions = within(
      canvas.getByRole("listbox", { name: "Plugin permissions" }),
    );
    const approve = canvas.getByRole("button", { name: "Allow access" });
    await expect(vaults.queryByRole("option", { name: "Archive" })).toBeNull();
    await expect(approve).toBeDisabled();
    await expect(
      canvas.getByRole("heading", { name: "Authorize application" }),
    ).toBeVisible();
    await expect(canvas.getByText("mcp:read", { exact: true })).toBeVisible();
    await expect(
      canvas.getByText("offline_access", { exact: true }),
    ).toBeVisible();
    await expect(canvas.getByRole("note")).toHaveTextContent(
      "You can disconnect this application",
    );
    await expect(permissions.getAllByRole("option")).toHaveLength(4);

    await userEvent.type(search, "proj");
    await expect(vaults.queryByRole("option", { name: "Notes" })).toBeNull();
    await userEvent.keyboard("{Enter}");
    await expect(
      vaults.getByRole("option", { name: "Projects" }),
    ).toHaveAttribute("aria-selected", "true");
    await userEvent.clear(search);
    await userEvent.click(await vaults.findByRole("option", { name: "Notes" }));
    await expect(
      vaults.getAllByRole("option", { selected: true }),
    ).toHaveLength(2);
    await expect(approve).toBeEnabled();

    const permissionSearch = canvas.getByRole("combobox", {
      name: "Plugin permissions",
    });
    await userEvent.type(permissionSearch, "tasks");
    await expect(
      permissions.queryByRole("option", { name: /skills:read/ }),
    ).toBeNull();
    await userEvent.click(
      permissions.getByRole("option", { name: /tasks:read/ }),
    );
    await userEvent.click(
      permissions.getByRole("option", { name: /tasks:write/ }),
    );
    await expect(
      permissions.getAllByRole("option", { selected: true }),
    ).toHaveLength(2);

    await userEvent.click(
      canvas.getByRole("button", { name: "Remove Projects" }),
    );
    await userEvent.click(canvas.getByRole("button", { name: "Remove Notes" }));
    await expect(approve).toBeDisabled();
    await expect(
      permissions.getAllByRole("option", { selected: true }),
    ).toHaveLength(2);
    await userEvent.click(
      canvas.getByRole("button", { name: "Remove tasks:read" }),
    );
    await userEvent.click(
      canvas.getByRole("button", { name: "Remove tasks:write" }),
    );
    await userEvent.clear(permissionSearch);
  },
};

export const ConsentDark: Story = {
  ...Consent,
  globals: { colorMode: "dark" },
};
