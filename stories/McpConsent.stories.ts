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

export const Consent: StoryObj<typeof meta> = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const notes = await canvas.findByRole("checkbox", { name: "Notes" });
    const approve = canvas.getByRole("button", { name: "Allow access" });
    const readTasks = canvas.getByRole("checkbox", { name: "tasks:read" });

    await expect(
      canvas.queryByRole("checkbox", { name: "Archive" }),
    ).toBeNull();
    await expect(approve).toBeDisabled();
    await expect(readTasks).not.toBeChecked();
    await userEvent.click(notes);
    await expect(approve).toBeEnabled();
    await userEvent.click(readTasks);
    await expect(readTasks).toBeChecked();
    await userEvent.click(readTasks);
    await userEvent.click(notes);
    await expect(approve).toBeDisabled();
  },
};
