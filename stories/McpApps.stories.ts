import type { Meta, StoryObj } from "@storybook/svelte-vite";
import { expect, waitFor, within } from "storybook/test";
import McpAppConversation from "./fixtures/McpAppConversation.svelte";

const meta = {
  title: "Shell/MCP Apps",
  component: McpAppConversation,
  args: { kind: "notes", theme: "light" },
  render: (args, context) => ({
    Component: McpAppConversation,
    props: {
      ...args,
      theme: context.globals.colorMode === "dark" ? "dark" : "light",
    },
  }),
  tags: ["visual-pending", "!autodocs"],
} satisfies Meta<typeof McpAppConversation>;

export default meta;
type Story = StoryObj<typeof meta>;

const loaded: Story["play"] = async ({ canvasElement }) => {
  const canvas = within(canvasElement);
  const surface = canvas.getByTestId("mcp-app");
  await waitFor(() => expect(surface).toHaveAttribute("data-ready", "true"), {
    timeout: 15_000,
  });
  const frame = surface.querySelector("iframe")!;
  await expect(frame).toBeVisible();
  expect(frame.getBoundingClientRect().height).toBeGreaterThan(100);
};

export const Notes: Story = { play: loaded };
export const Tasks: Story = { args: { kind: "tasks" }, play: loaded };
