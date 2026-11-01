import { expect, type Page } from "@playwright/test";
import { manageVaults } from "./tasks-editor-controls";

export async function signInHostedOwner(page: Page, password: string) {
  await page.getByLabel("Username").fill("owner");
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Your vaults", exact: true }),
  ).toBeVisible();
}

/** Leave through the guarded UI so native editing claims are really released. */
export async function signOutHostedWorkspace(page: Page) {
  if (await page.getByLabel("Username").isVisible()) return;
  const closeSettings = page.getByRole("button", {
    name: "Close settings",
    exact: true,
  });
  if (await closeSettings.isVisible()) await closeSettings.click();
  if (
    !(await page
      .getByRole("heading", { name: "Your vaults", exact: true })
      .isVisible())
  )
    await manageVaults(page);
  await page.getByRole("button", { name: "Sign out", exact: true }).click();
  await expect(page.getByLabel("Username")).toBeVisible();
}
