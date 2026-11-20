import type { Meta, StoryObj } from "@storybook/svelte-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { disposeServerHostFixtures } from "./fixtures/ServerHostFixture.svelte";
import ServerTerminalFixture, {
  terminalReviewFor,
} from "./fixtures/ServerTerminalFixture.svelte";
import type { TerminalPlugin } from "@lapis-notes/lapis-plugin-terminal";
import { createTerminalBackend } from "./fixtures/terminal-backend";
import type { TerminalScenario } from "./fixtures/terminal-output";

const render = (scenario: TerminalScenario) => () => ({
  Component: ServerTerminalFixture,
  props: { backend: createTerminalBackend(scenario) },
});
const meta = {
  title: "Shell/Server Terminal",
  component: ServerTerminalFixture,
  args: { backend: createTerminalBackend("ready") },
  render: render("ready"),
  tags: ["visual-pending", "!autodocs"],
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component:
          "Server-only appearance review in the production hosted workspace and Ghostty terminal. Shell output is a disposable fixture previewing the proposed Bash profile; it does not spawn processes. Try ls, pwd, cd notes, cd .., false, or clear. Desktop is unchanged.",
      },
    },
  },
  beforeEach: () => disposeServerHostFixtures,
} satisfies Meta<typeof ServerTerminalFixture>;
export default meta;
type Story = StoryObj<typeof meta>;

async function ready(
  canvasElement: HTMLElement,
  status: "success" | "failure" = "success",
) {
  const canvas = within(canvasElement);
  await userEvent.click(
    await canvas.findByRole("button", {
      name: "Open Notes owner · server-hosted",
    }),
  );
  await waitFor(
    () =>
      expect(
        canvasElement.querySelector('[data-app-shell-ready="true"]'),
      ).toBeInTheDocument(),
    { timeout: 30_000 },
  );
  await userEvent.click(canvas.getByRole("button", { name: "Welcome.md" }));
  const leftSidebar = canvas.queryByRole("button", {
    name: "Close left sidebar",
  });
  if (leftSidebar) await userEvent.click(leftSidebar);
  const rightSidebar = canvas.queryByRole("button", {
    name: "Close right sidebar",
  });
  if (rightSidebar) await userEvent.click(rightSidebar);
  expect(
    canvas.getByRole("button", { name: "Open left sidebar" }),
  ).toBeVisible();
  await waitFor(
    () =>
      expect(
        canvas.getByTestId("terminal-surface").querySelector("canvas"),
      ).toBeVisible(),
    { timeout: 30_000 },
  );
  await waitFor(() => {
    expect(document.fonts.check('400 14px "JetBrains Mono"')).toBe(true);
    expect(document.fonts.check('700 14px "JetBrains Mono"')).toBe(true);
  });
  await waitFor(() => {
    const surface = canvas.getByTestId("terminal-surface");
    const bitmap = surface.querySelector("canvas")!;
    expect(
      Math.abs(
        bitmap.getBoundingClientRect().right -
          surface.getBoundingClientRect().right,
      ),
    ).toBeLessThan(1);
    expect(
      Math.abs(
        surface.getBoundingClientRect().right -
          canvas.getByTestId("terminal-panel").getBoundingClientRect().right,
      ),
    ).toBeLessThan(1);
  });
  const bitmap = canvas
    .getByTestId("terminal-surface")
    .querySelector("canvas")!;
  await waitFor(() => {
    const pixels = bitmap
      .getContext("2d")!
      .getImageData(0, 0, bitmap.width, bitmap.height).data;
    // Ghostty's muted cyan/magenta/blue palette has channel differences below
    // 80. A saturation cutoff measured only the green arrow, whose rasterized
    // area differs across platforms. Require each intended ANSI colour instead.
    const expectedColours = [
      { label: "cyan path", rgb: [138, 190, 183], count: 0 },
      { label: "magenta UUID", rgb: [178, 148, 187], count: 0 },
      { label: "blue directory", rgb: [129, 162, 190], count: 0 },
      {
        label: `${status} status`,
        rgb: status === "failure" ? [204, 102, 102] : [181, 189, 104],
        count: 0,
      },
    ];
    for (let i = 0; i < pixels.length; i += 4) {
      if (pixels[i + 3] <= 200) continue;
      for (const colour of expectedColours)
        if (
          colour.rgb.every((channel, offset) => pixels[i + offset] === channel)
        )
          colour.count++;
    }
    for (const colour of expectedColours)
      expect(colour.count, `visible ${colour.label} pixels`).toBeGreaterThan(0);
    expect(
      expectedColours.reduce((total, colour) => total + colour.count, 0),
    ).toBeGreaterThan(100);
  });
  await waitFor(() => {
    const surface = canvas.getByTestId("terminal-surface");
    const reference = document.createElement("canvas").getContext("2d")!;
    reference.fillStyle = getComputedStyle(surface).backgroundColor;
    reference.fillRect(0, 0, 1, 1);
    const expected = [...reference.getImageData(0, 0, 1, 1).data].join(",");
    const pixels = bitmap
      .getContext("2d")!
      .getImageData(0, 0, bitmap.width, bitmap.height).data;
    const counts = new Map<string, number>();
    for (let index = 0; index < pixels.length; index += 4) {
      const key = [...pixels.subarray(index, index + 4)].join(",");
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    const dominant = [...counts].sort((a, b) => b[1] - a[1])[0][0];
    expect(dominant).toBe(expected);
  });
}

export const CompactPrompt: Story = {
  play: async ({ canvasElement }) => ready(canvasElement),
};
export const CompactPromptDark: Story = {
  ...CompactPrompt,
  globals: { colorMode: "dark" },
};
export const FailedCommand: Story = {
  render: render("failure"),
  play: async ({ canvasElement }) => ready(canvasElement, "failure"),
};
export const FailedCommandDark: Story = {
  ...FailedCommand,
  globals: { colorMode: "dark" },
};
export const NestedDirectory: Story = {
  render: render("nested"),
  play: CompactPrompt.play,
};
export const NestedDirectoryDark: Story = {
  ...NestedDirectory,
  globals: { colorMode: "dark" },
};

export const MultipleTerminals: Story = {
  play: async ({ canvasElement }) => {
    await ready(canvasElement);
    const canvas = within(canvasElement);
    const review = terminalReviewFor(canvasElement);
    const app = review.getApp();
    const { sessions } = review;
    const leaves = () => app.workspace.getLeavesOfType("terminal");
    await waitFor(() => expect(sessions.size).toBe(1));
    const first = leaves()[0];
    const firstSession = first.view.getState().sessionId as string;
    const plugin = app.plugins.plugins.get("terminal") as TerminalPlugin;
    await plugin.sessions.write(firstSession, "cd notes\r");
    await waitFor(() =>
      expect(sessions.get(firstSession)?.text).toContain("/content/notes"),
    );
    app.workspace.setBottomPanelOpen(false);
    await app.commands.executeCommand("terminal:open-terminal");
    await waitFor(() =>
      expect(app.workspace.bottomPanel.collapsed).toBe(false),
    );
    expect(leaves(), "hidden terminal should be reused").toHaveLength(1);
    expect(first.view.getState().sessionId).toBe(firstSession);
    await app.commands.executeCommand("terminal:open-terminal");
    await waitFor(() => expect(sessions.size).toBe(2));
    expect(leaves()).toHaveLength(2);
    expect(leaves().map((leaf) => leaf.view.getDisplayText())).toEqual([
      "Terminal 1",
      "Terminal 2",
    ]);
    const second = leaves()[1];
    await waitFor(() =>
      expect(second.view.getState().sessionId).toEqual(expect.any(String)),
    );
    const secondSession = second.view.getState().sessionId as string;
    expect(secondSession).not.toBe(firstSession);
    await plugin.sessions.write(secondSession, "pwd\r");
    await waitFor(() =>
      expect(sessions.get(secondSession)?.text).toContain(
        "92b3bbf0-07f6-468e-a424-06d7bbdba797/content",
      ),
    );
    expect(sessions.get(secondSession)?.text).not.toContain("/content/notes");
    await app.workspace.revealLeaf(first);
    await userEvent.click(canvas.getByRole("button", { name: "New Terminal" }));
    await waitFor(() => expect(sessions.size).toBe(3));
    expect(leaves().map((leaf) => leaf.view.getDisplayText())).toEqual([
      "Terminal 1",
      "Terminal 2",
      "Terminal 3",
    ]);
    await app.workspace.revealLeaf(first);
    expect(first.view.getState().sessionId).toBe(firstSession);
    expect(sessions.get(firstSession)?.text).toContain("/content/notes");
  },
};
export const MultipleTerminalsDark: Story = {
  ...MultipleTerminals,
  globals: { colorMode: "dark" },
};
