import type { Meta, StoryObj } from "@storybook/svelte-vite";
import ServerCredentialForm from "../web/src/ServerCredentialForm.svelte";

const meta = {
  title: "Server/Credential form",
  component: ServerCredentialForm,
} satisfies Meta<typeof ServerCredentialForm>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Empty: Story = {};
