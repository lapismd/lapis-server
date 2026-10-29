import { expect, test } from "vitest";
import { render, waitFor } from "@testing-library/svelte";
import ServerNotesWorkspace from "../../stories/ServerNotesWorkspace.svelte";

test("hosted Notes UI loads WorkspaceShell from the published package", async () => {
  const view = render(ServerNotesWorkspace);
  const host = await view.findByTestId("hosted-notes-workspace");
  expect(host.getAttribute("data-shell-export")).toBe("function");
  await waitFor(() => {
    expect(
      view.container.querySelector(
        '[data-ui-component="lapis-workspace-shell"]',
      ),
    ).toBeTruthy();
  });
});
