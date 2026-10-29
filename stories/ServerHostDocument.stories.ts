import type { Meta, StoryObj } from "@storybook/svelte-vite";
import ServerHostDocument from "./ServerHostDocument.svelte";

const meta = {
  title: "Server/Host document",
  component: ServerHostDocument,
} satisfies Meta<typeof ServerHostDocument>;

export default meta;

type Story = StoryObj<typeof meta>;

export const RustServer: Story = {};
