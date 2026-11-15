import type { Meta, StoryObj } from "@storybook/svelte-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import SyncAccountConsent from "../web/src/SyncAccountConsent.svelte";
import { RustHttpClient } from "../web/src/hosted/rust/client";

const requestId = "82791527-4c73-4820-84f4-5db3f7ee5400";
function host(status = "pending", lostResponse = false) {
  return new RustHttpClient("", async (url, init) => {
    if (String(url).endsWith("/system/info"))
      return Response.json({ protocol_major: 3, browser_auth: true });
    if (String(url).endsWith("/auth/current"))
      return Response.json({
        username: "Alex",
        csrf_token: "isolated-story-csrf",
      });
    if (String(url).endsWith("/decision")) {
      if (lostResponse) throw new Error("Connection interrupted");
      return Response.json({
        approved: JSON.parse(String(init?.body)).approve,
      });
    }
    return Response.json({
      id: requestId,
      client_id: "e09dd6f5-9bbc-4faa-aefc-b55fa4eef854",
      client_name: "Alex’s MacBook",
      expires_at: Date.now() + 60_000,
      status,
    });
  });
}

const meta = {
  title: "Shell/Sync account consent",
  component: SyncAccountConsent,
  tags: ["autodocs", "visual-pending"],
  args: { requestId, host: host() },
  argTypes: { host: { control: false }, requestId: { control: false } },
  parameters: {
    layout: "fullscreen",
    docs: {
      story: { inline: false, height: "700px" },
      source: {
        type: "code",
        language: "svelte",
        code: '<script>\n  import SyncAccountConsent from "./web/src/SyncAccountConsent.svelte";\n  const requestId = new URLSearchParams(location.search).get("sync-connect") ?? "";\n</script>\n\n<SyncAccountConsent {requestId} />',
      },
    },
  },
} satisfies Meta<typeof SyncAccountConsent>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Review: Story = {
  render: () => ({
    Component: SyncAccountConsent,
    props: { requestId, host: host() },
  }),
  parameters: {
    docs: {
      description: {
        story:
          "Review a named device and approve its discovery-only connection with the keyboard.",
      },
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("button", { name: "Approve connection" });
    expect(
      canvas.getByRole("heading", { name: "Alex’s MacBook" }),
    ).toBeVisible();
    await userEvent.tab();
    expect(canvas.getByRole("button", { name: "Reject" })).toHaveFocus();
    await userEvent.tab();
    expect(
      canvas.getByRole("button", { name: "Approve connection" }),
    ).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(await canvas.findByRole("status")).toHaveTextContent(
      "Connection approved",
    );
  },
};

export const Expired: Story = {
  render: () => ({
    Component: SyncAccountConsent,
    props: { requestId, host: host("expired") },
  }),
  parameters: {
    docs: {
      description: { story: "An expired request offers no approval action." },
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await waitFor(() =>
      expect(canvas.getByRole("status")).toHaveTextContent("expired"),
    );
    expect(
      canvas.queryByRole("button", { name: "Approve connection" }),
    ).toBeNull();
  },
};

export const UnknownOutcome: Story = {
  render: () => ({
    Component: SyncAccountConsent,
    props: { requestId, host: host("pending", true) },
  }),
  parameters: {
    docs: {
      description: {
        story:
          "A lost response requires fresh inspection before another decision.",
      },
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", { name: "Approve connection" }),
    );
    expect(await canvas.findByRole("alert")).toHaveTextContent(
      "Reload this page to inspect",
    );
    expect(
      canvas.queryByRole("button", { name: "Approve connection" }),
    ).toBeNull();
  },
};
