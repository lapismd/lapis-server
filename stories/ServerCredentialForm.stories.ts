import type { Meta, StoryObj } from "@storybook/svelte-vite";
import { expect, waitFor, within } from "storybook/test";
import ServerCredentialForm from "../web/src/ServerCredentialForm.svelte";
import { expectShellFillsViewport } from "./shell-layout";

const meta = {
  title: "Shell/Credential form",
  component: ServerCredentialForm,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof ServerCredentialForm>;

export default meta;

type Story = StoryObj<typeof meta>;

export const SignIn: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await waitFor(() => {
      expect(canvas.getByTestId("server-credential-workspace")).toBeTruthy();
      expect(
        canvasElement.querySelector(
          '[data-ui-component="lapis-workspace-shell"]',
        ),
      ).toBeTruthy();
      const input = canvasElement.querySelector(
        '[data-ui-component="input"]',
      ) as HTMLElement | null;
      expect(input).toBeTruthy();
      expect(getComputedStyle(input!).borderRadius).not.toBe("0px");
      expect(
        canvasElement.querySelector('[data-ui-component="card"]'),
      ).toBeTruthy();
      expectShellFillsViewport(canvasElement);
    });
  },
};
