import type { Meta, StoryObj } from "@storybook/svelte-vite";
import { expect, waitFor, within } from "storybook/test";
import ServerNotesWorkspace from "./ServerNotesWorkspace.svelte";

const meta = {
  title: "Server/Notes workspace",
  component: ServerNotesWorkspace,
} satisfies Meta<typeof ServerNotesWorkspace>;

export default meta;

type Story = StoryObj<typeof meta>;

export const HostedNotes: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await waitFor(() => {
      expect(canvas.getByTestId("hosted-notes-workspace")).toHaveAttribute(
        "data-shell-export",
        "function",
      );
      expect(
        canvasElement.querySelector(
          '[data-ui-component="lapis-workspace-shell"]',
        ),
      ).toBeTruthy();
    });
  },
};
