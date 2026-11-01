import { expect, type Page } from "@playwright/test";

export async function inspectRetainedTaskUpdate(page: Page, requestId: string) {
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
    .fill("Inspect interrupted task update");
  await palette
    .getByRole("option", { name: /Inspect interrupted task update/ })
    .click();
  await page
    .getByRole("option", {
      name: new RegExp(`Hosted\\.md · complete.*${requestId}`),
    })
    .click();
  await expect(
    page.getByText(/Native journal recorded completed/).first(),
  ).toBeVisible();
  await expect(
    page.getByText(/Current files may have changed/).first(),
  ).toBeVisible();
  await expect(
    page.getByRole("option", {
      name: "Open current task: Hosted.md",
      exact: true,
    }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
}
