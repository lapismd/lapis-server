import { expect, type Page } from "@playwright/test";

export async function openTaskSource(page: Page) {
  await page
    .getByRole("button", { name: "Open command palette", exact: true })
    .click();
  const palette = page.getByRole("dialog", {
    name: "Command Palette",
    exact: true,
  });
  await palette.getByRole("tab", { name: "Files", exact: true }).click();
  await palette
    .getByRole("combobox", { name: "Search commands", exact: true })
    .fill("Hosted.md");
  await palette.getByRole("option", { name: /Hosted\.md/ }).click();
  const editor = page.locator(".cm-content").first();
  await expect(editor).toBeVisible();
  await expect(editor).toHaveAttribute("contenteditable", "true");
  return editor;
}

/**
 * Blink implements document-end caret movement as a native editing binding,
 * not a cross-platform accelerator: macOS maps Meta+ArrowDown while other
 * platforms map Control+End. Neither CodeMirror nor the app binds either
 * key, so a hard-coded Control+End is a silent no-op on macOS and the next
 * keystrokes land at the previous click position — inside the raw
 * frontmatter of a Source-mode task document.
 */
export async function moveCaretToEndOfDocument(page: Page): Promise<void> {
  await page.keyboard.press(
    process.platform === "darwin" ? "Meta+ArrowDown" : "Control+End",
  );
}

export async function manageVaults(page: Page) {
  await page.getByRole("button", { name: /Current workspace:/ }).click();
  await page
    .getByRole("menuitem", { name: "Manage Vaults", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Your vaults", exact: true }),
  ).toBeVisible();
}

export async function showEditingProblems(page: Page) {
  await page
    .getByRole("button", { name: "Open command palette", exact: true })
    .click();
  const palette = page.getByRole("dialog", {
    name: "Command Palette",
    exact: true,
  });
  await palette.getByRole("tab", { name: "Actions", exact: true }).click();
  await palette
    .getByRole("combobox", { name: "Search commands", exact: true })
    .fill("Problems");
  await palette.getByRole("option", { name: /Problems/ }).click();
  return page.getByText(/Unresolved editing claim\./).first();
}
