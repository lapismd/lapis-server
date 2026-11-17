import type { Meta, StoryObj } from "@storybook/svelte-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import ServerVaultHost from "../web/src/ServerVaultHost.svelte";
import ServerHostFixture, {
  disposeServerHostFixtures,
} from "./fixtures/ServerHostFixture.svelte";
import {
  createHostedFixture,
  type HostedScenario,
} from "./fixtures/hosted-backend";
import { expectShellFillsViewport } from "./shell-layout";
import { createRepositoryHostedFixture } from "./fixtures/repository-backend";

const renderScenario =
  (scenario: HostedScenario, restoreVaultId?: string) => () => ({
    Component: ServerHostFixture,
    props: {
      backend: createHostedFixture(scenario).backend,
      ...(restoreVaultId ? { restoreVaultId } : {}),
    },
  });
const meta = {
  title: "Shell/Hosted application",
  component: ServerVaultHost,
  args: { backend: createHostedFixture("login").backend },
  render: renderScenario("login"),
  tags: ["autodocs", "visual-pending"],
  argTypes: { backend: { control: false }, restoration: { control: false } },
  parameters: {
    layout: "fullscreen",
    docs: {
      story: { inline: false, height: "700px" },
      canvas: { className: "server-docs-canvas" },
      source: {
        type: "code",
        language: "tsx",
        code: '<script>\n  import ServerVaultHost from "./web/src/ServerVaultHost.svelte";\n  import { createRustHostedBackend } from "./web/src/hosted/rust/backend";\n  const backend = createRustHostedBackend(undefined, { nativeMetadata: true, nativeHistory: true, nativeTasks: true });\n</script>\n\n<ServerVaultHost {backend} />',
      },
    },
  },
  beforeEach: () => disposeServerHostFixtures,
} satisfies Meta<typeof ServerVaultHost>;
export default meta;
type Story = StoryObj<typeof meta>;

async function expectLogin(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  await canvas.findByRole("heading", { name: "Sign in" });
  await waitFor(() => {
    const card = canvasElement.querySelector(".server-login-card")!;
    const box = card.getBoundingClientRect();
    const viewport = canvasElement.ownerDocument.documentElement;
    expect(
      Math.abs(box.x + box.width / 2 - viewport.clientWidth / 2),
    ).toBeLessThan(1);
    expect(
      Math.abs(box.y + box.height / 2 - viewport.clientHeight / 2),
    ).toBeLessThan(1);
    const logo = canvasElement.querySelector<HTMLImageElement>(
      "[data-server-login-logo]",
    )!;
    expect(logo.naturalWidth).toBeGreaterThan(0);
  });
  expect(
    canvasElement.querySelector('[data-ui-component="lapis-workspace-shell"]'),
  ).toBeNull();
  await userEvent.tab();
  expect(canvas.getByLabelText("Username")).toHaveFocus();
}

async function openWorkspace(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  await userEvent.click(
    await canvas.findByRole("button", {
      name: "Open Notes owner · server-hosted",
    }),
  );
  await waitFor(
    () =>
      expect(
        canvasElement.querySelector(
          '[data-notes-session-ready="true"] [data-app-shell-ready="true"]',
        ),
      ).toBeInTheDocument(),
    { timeout: 20_000 },
  );
  expectShellFillsViewport(canvasElement);
  const narrow = canvasElement.ownerDocument.documentElement.clientWidth < 600;
  if (narrow) {
    await userEvent.click(
      canvas.getByRole("button", { name: "Open left sidebar" }),
    );
    expect(canvas.getByRole("button", { name: /^Welcome\.md$/ })).toBeVisible();
  } else {
    expect(canvas.getByText("Server DB", { exact: true })).toBeVisible();
  }
  await userEvent.click(
    await canvas.findByRole("button", { name: /^Welcome\.md$/ }),
  );
  if (narrow) {
    for (const side of ["left", "right"]) {
      const close = canvas.queryByRole("button", {
        name: `Close ${side} sidebar`,
      });
      if (close) await userEvent.click(close);
    }
  }
  await waitFor(
    () =>
      expect(
        canvasElement.querySelector('.cm-content[contenteditable="true"]'),
      ).toBeVisible(),
    { timeout: 20_000 },
  );
}

export const Login: Story = {
  play: async ({ canvasElement }) => {
    await expectLogin(canvasElement);
  },
};
export const LoginDark: Story = { ...Login, globals: { colorMode: "dark" } };
export const LoginError: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByLabelText("Username");
    await userEvent.type(canvas.getByLabelText("Username"), "owner");
    await userEvent.type(canvas.getByLabelText("Password"), "incorrect");
    await userEvent.click(canvas.getByRole("button", { name: "Sign in" }));
    await canvas.findByRole("alert");
  },
};
export const LoginErrorDark: Story = {
  ...LoginError,
  globals: { colorMode: "dark" },
};
export const Connecting: Story = {
  render: renderScenario("loading"),
  play: async ({ canvasElement }) => {
    await within(canvasElement).findByRole("heading", { name: "Connecting" });
  },
};
export const ConnectingDark: Story = {
  ...Connecting,
  globals: { colorMode: "dark" },
};
export const ProtocolMismatch: Story = {
  render: renderScenario("protocol"),
  play: async ({ canvasElement }) => {
    await within(canvasElement).findByRole("heading", {
      name: "This Lapis tab is out of date",
    });
  },
};
export const ProtocolMismatchDark: Story = {
  ...ProtocolMismatch,
  globals: { colorMode: "dark" },
};
export const EmptyVaults: Story = {
  render: renderScenario("empty"),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("heading", { name: "Your vaults" });
    expect(
      canvas.getByRole("button", { name: /^Create New Vault/ }),
    ).toBeEnabled();
  },
};
export const EmptyVaultsDark: Story = {
  ...EmptyVaults,
  globals: { colorMode: "dark" },
};
export const VaultChooser: Story = {
  render: renderScenario("vaults"),
  play: async ({ canvasElement }) => {
    await within(canvasElement).findByRole("button", {
      name: "Open Notes owner · server-hosted",
    });
  },
};
export const VaultChooserDark: Story = {
  ...VaultChooser,
  globals: { colorMode: "dark" },
};
export const RestoredWorkspace: Story = {
  render: renderScenario("vaults", "notes"),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await waitFor(
      () =>
        expect(
          canvasElement.querySelector('[data-notes-session-ready="true"]'),
        ).toBeInTheDocument(),
      { timeout: 20_000 },
    );
    expect(canvas.getByText("Server DB", { exact: true })).toBeVisible();
    expect(
      canvas.queryByRole("heading", { name: "Your vaults" }),
    ).not.toBeInTheDocument();
    expectShellFillsViewport(canvasElement);
  },
};
export const RestoredWorkspaceDark: Story = {
  ...RestoredWorkspace,
  globals: { colorMode: "dark" },
};
export const UnavailableRememberedVault: Story = {
  render: renderScenario("vaults", "missing"),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("heading", { name: "Your vaults" });
    expect((await canvas.findByRole("alert")).textContent).toContain(
      "previously selected vault is unavailable",
    );
    expect(
      canvas.getByRole("button", { name: "Open Notes owner · server-hosted" }),
    ).toBeEnabled();
  },
};
export const UnavailableRememberedVaultDark: Story = {
  ...UnavailableRememberedVault,
  globals: { colorMode: "dark" },
};
export const VaultActions: Story = {
  render: renderScenario("vaults"),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", { name: "Open actions for Notes" }),
    );
    const page = within(canvasElement.ownerDocument.body);
    await userEvent.click(page.getByRole("menuitem", { name: "Rename..." }));
    const dialog = page.getByRole("dialog", { name: "Rename vault" });
    const input = within(dialog).getByLabelText("Name");
    await userEvent.clear(input);
    await userEvent.type(input, "Renamed notes");
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await canvas.findByRole("button", {
      name: "Open Renamed notes owner · server-hosted",
    });
  },
};
export const VaultActionsDark: Story = {
  ...VaultActions,
  globals: { colorMode: "dark" },
};
export const Opening: Story = {
  render: renderScenario("opening"),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", {
        name: "Open Notes owner · server-hosted",
      }),
    );
    await canvas.findByRole("heading", { name: "Opening vault" });
  },
};
export const OpeningDark: Story = {
  ...Opening,
  globals: { colorMode: "dark" },
};
export const StartupRetry: Story = {
  render: renderScenario("startup-failure"),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", {
        name: "Open Notes owner · server-hosted",
      }),
    );
    await canvas.findByText("Lapis Notes could not start");
    await userEvent.click(canvas.getByRole("button", { name: "Retry" }));
    await waitFor(
      () =>
        expect(
          canvasElement.querySelector(
            '[data-notes-session-ready="true"] [data-app-shell-ready="true"]',
          ),
        ).toBeInTheDocument(),
      { timeout: 20_000 },
    );
    expectShellFillsViewport(canvasElement);
  },
};
export const StartupRetryDark: Story = {
  ...StartupRetry,
  globals: { colorMode: "dark" },
};
export const Workspace: Story = {
  render: renderScenario("vaults"),
  play: async ({ canvasElement }) => {
    await openWorkspace(canvasElement);
  },
};
export const WorkspaceDark: Story = {
  ...Workspace,
  globals: { colorMode: "dark" },
};
export const Settings: Story = {
  render: renderScenario("vaults"),
  play: async ({ canvasElement }) => {
    await openWorkspace(canvasElement);
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Settings" }));
    await within(canvasElement.ownerDocument.body).findByRole("button", {
      name: "Close settings",
    });
  },
};
export const SettingsDark: Story = {
  ...Settings,
  globals: { colorMode: "dark" },
};

export const RepositoryRemotes: Story = {
  render: () => ({
    Component: ServerHostFixture,
    props: { backend: createRepositoryHostedFixture().backend },
  }),
  play: async ({ canvasElement }) => {
    await openWorkspace(canvasElement);
    const canvas = within(canvasElement);
    await canvas.findByRole("button", { name: "Expand repository status" });
    await userEvent.click(canvas.getByRole("button", { name: "Settings" }));
    const page = within(canvasElement.ownerDocument.body);
    await userEvent.click(
      await page.findByRole("button", {
        name: "Repository History",
      }),
    );
    await page.findByRole("heading", { name: "Remotes and credentials" });
    for (const target of ["Vault backup", "Original repository"]) {
      expect(await page.findByRole("region", { name: target })).toBeVisible();
      const label = target.toLowerCase();
      expect(
        page.getByRole("button", { name: `Fetch ${label}` }),
      ).toBeEnabled();
      expect(
        page.getByRole("button", {
          name: `Review push to ${label}`,
        }),
      ).toBeEnabled();
    }
    const original = within(
      page.getByRole("region", { name: "Original repository" }),
    );
    await userEvent.click(
      original.getByRole("button", {
        name: "Fetch original repository",
      }),
    );
    await waitFor(() =>
      expect(original.getByLabelText("Incoming commits")).toHaveTextContent(
        "0",
      ),
    );
    const backup = within(page.getByRole("region", { name: "Vault backup" }));
    expect(backup.getByLabelText("Incoming commits")).toHaveTextContent("2");
    expect(
      page.getByRole("switch", {
        name: "Synchronize the vault backup automatically",
      }),
    ).not.toBeChecked();
    expect(
      page.queryByRole("button", { name: "Pull from upstream" }),
    ).not.toBeInTheDocument();
  },
};
export const RepositoryRemotesDark: Story = {
  ...RepositoryRemotes,
  globals: { colorMode: "dark" },
};

export const RepositoryNativeRecovery: Story = {
  render: () => ({
    Component: ServerHostFixture,
    props: {
      backend: createRepositoryHostedFixture({
        revision: 11,
        enabled: false,
        busy: false,
        phase: "attention",
        recovery: true,
        pendingApply: "retained-native-apply",
      }).backend,
    },
  }),
  play: async ({ canvasElement }) => {
    await openWorkspace(canvasElement);
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Settings" }));
    const page = within(canvasElement.ownerDocument.body);
    await userEvent.click(
      await page.findByRole("button", { name: "Repository History" }),
    );
    await page.findByRole("heading", { name: "Remotes and credentials" });
    expect(
      await page.findByText("Automatic synchronization", { exact: true }),
    ).toBeVisible();
    const resume = await page.findByRole("button", {
      name: "Retry applying merged files",
    });
    expect(resume).toBeEnabled();
    expect(page.getByRole("button", { name: "Sync now" })).toBeDisabled();
    await userEvent.click(resume);
    await waitFor(() =>
      expect(
        page.queryByRole("button", { name: "Retry applying merged files" }),
      ).not.toBeInTheDocument(),
    );
  },
};
export const RepositoryNativeRecoveryDark: Story = {
  ...RepositoryNativeRecovery,
  globals: { colorMode: "dark" },
};

export const RepositoryFooterStatus: Story = {
  render: RepositoryRemotes.render,
  play: async ({ canvasElement }) => {
    await openWorkspace(canvasElement);
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", { name: "Expand repository status" }),
    );
    const footer = within(canvas.getByLabelText("Repository working copy"));
    expect(footer.getByText("Saved locally", { exact: true })).toBeVisible();
    expect(footer.getByText("+7", { exact: true })).toBeVisible();
    expect(footer.getByText("-2", { exact: true })).toBeVisible();
    expect(footer.getByLabelText("Incoming commits")).toHaveTextContent("2");
    expect(footer.getByLabelText("Outgoing commits")).toHaveTextContent("1");
    await userEvent.click(
      footer.getByRole("button", { name: /^Working copy details:/ }),
    );
    const page = within(canvasElement.ownerDocument.body);
    const details = await page.findByRole("region", {
      name: "Working copy details",
    });
    const content = within(details);
    expect(
      content.getByRole("button", {
        name: "Fetch original repository",
      }),
    ).toBeEnabled();
    expect(
      content.getByRole("button", {
        name: "Review push to original repository",
      }),
    ).toBeEnabled();
    expect(
      content.getByRole("button", {
        name: "Manage remotes for original repository",
      }),
    ).toBeEnabled();
    expect(content.queryByRole("combobox")).not.toBeInTheDocument();
    const box = details.getBoundingClientRect();
    expect(box.left).toBeGreaterThanOrEqual(0);
    expect(box.right).toBeLessThanOrEqual(
      canvasElement.ownerDocument.documentElement.clientWidth,
    );
    await userEvent.keyboard("{Escape}");
    await userEvent.click(
      footer.getByRole("button", { name: "Collapse repository status" }),
    );
    expect(
      footer.getByText("Saved locally", { exact: true }),
    ).not.toBeVisible();
  },
};
export const RepositoryFooterStatusDark: Story = {
  ...RepositoryFooterStatus,
  globals: { colorMode: "dark" },
};

export const ContentAccess: Story = {
  name: "Content API and MCP",
  render: renderScenario("vaults"),
  play: async ({ canvasElement }) => {
    await openWorkspace(canvasElement);
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Settings" }));
    const page = within(canvasElement.ownerDocument.body);
    const corePlugins = (
      await page.findByRole("heading", { name: /^Core plugins$/ })
    ).closest("section")!;
    await userEvent.click(
      within(corePlugins).getByRole("button", { name: "Content API and MCP" }),
    );
    const enabled = await page.findByRole("switch", {
      name: "Enable API and MCP access",
    });
    await userEvent.click(enabled);
    await page.findByText("Enabled · scoped access");
    const label = page.getByRole("textbox", { name: "Credential label" });
    await userEvent.clear(label);
    await userEvent.type(label, "Story client");
    await userEvent.click(
      page.getByRole("button", { name: "Create a scoped credential" }),
    );
    await page.findByRole("button", { name: /^Copy full credential:/ });
    await page.findByText("Story client", { exact: true });
    await userEvent.click(
      page.getByRole("button", { name: "Hide the one-time credential" }),
    );
    await page.findByText("No credential to display");
    await userEvent.click(page.getByRole("button", { name: "Revoke" }));
    await waitFor(() =>
      expect(
        page.queryByText("Story client", { exact: true }),
      ).not.toBeInTheDocument(),
    );
    await userEvent.click(enabled);
    await page.findByText("Disabled", { exact: true });
  },
};

export const ContentAccessDark: Story = {
  ...ContentAccess,
  name: "Content API and MCP dark",
  globals: { colorMode: "dark" },
};
