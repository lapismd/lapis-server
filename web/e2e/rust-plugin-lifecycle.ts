import { expect, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";

export async function verifyInstalledPluginLifecycle(
  page: Page,
  vault: string,
  moduleUrl: string,
): Promise<void> {
  const enabledPlugins = async (): Promise<string[]> =>
    JSON.parse(
      await readFile(join(vault, ".lapis/community-plugins.json"), "utf8"),
    );
  await page
    .getByRole("switch", { name: "Disable Word Count", exact: true })
    .click();
  await expect(
    page.getByRole("switch", { name: "Enable Word Count", exact: true }),
  ).not.toBeChecked();
  await expect.poll(enabledPlugins).not.toContain("wordcount");
  await page
    .getByRole("switch", { name: "Enable Word Count", exact: true })
    .click();
  await expect(
    page.getByRole("switch", { name: "Disable Word Count", exact: true }),
  ).toBeChecked();
  await expect.poll(enabledPlugins).toContain("wordcount");

  expect((await page.request.get(moduleUrl)).status()).toBe(200);
  await page
    .getByRole("button", { name: "Uninstall wordcount", exact: true })
    .click();
  const confirmation = page.getByRole("alertdialog", {
    name: "Uninstall plugin?",
    exact: true,
  });
  await expect(confirmation).toBeVisible();
  await confirmation
    .getByRole("button", { name: "Uninstall Word Count", exact: true })
    .click();
  await expect(confirmation).not.toBeVisible();
  await expect(
    page.getByRole("button", { name: "Uninstall wordcount", exact: true }),
  ).not.toBeVisible();
  await expect.poll(enabledPlugins).not.toContain("wordcount");
  await expect
    .poll(async () => {
      const metadata = JSON.parse(
        await readFile(join(vault, ".lapis/installed-plugins.json"), "utf8"),
      );
      return metadata.plugins;
    })
    .not.toHaveProperty("wordcount");
  expect((await page.request.get(moduleUrl)).status()).toBe(404);
}
