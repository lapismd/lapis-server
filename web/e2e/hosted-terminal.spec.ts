import { expect, test } from "@playwright/test";
import { randomBytes } from "node:crypto";
import { mkdtemp, mkdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";

for (const renderer of ["preview", "development"] as const) {
  test(`bundled Terminal executes in its admitted vault (${renderer})`, async ({
    page,
  }) => {
    const repository = resolve(import.meta.dirname, "../..");
    await mkdir(join(repository, ".local"), { recursive: true });
    const root = await mkdtemp(join(repository, ".local/hosted-terminal-"));
    const origin = `http://localhost:${await unusedLoopbackPort()}`;
    const password = randomBytes(32).toString("base64url");
    const runtime = new RustServerRuntime(
      root,
      repository,
      origin,
      password,
      undefined,
      undefined,
      undefined,
      renderer,
    );
    const created: Array<{ session_id: string }> = [];
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("response", (response) => {
      if (
        response.url().endsWith("/terminal") &&
        response.request().postDataJSON()?.kind === "create"
      )
        void response.json().then((value) => created.push(value));
    });
    try {
      await runtime.start();
      await page.goto(origin);
      await page.getByLabel("Username").fill("owner");
      await page.getByLabel("Password").fill(password);
      await page.getByRole("button", { name: "Sign in", exact: true }).click();
      await page
        .getByRole("button", {
          name: "Open Rust vault owner · server-hosted",
          exact: true,
        })
        .click();
      await expect(
        page.getByRole("button", { name: "Open Chat", exact: true }),
      ).toBeVisible();
      await expect(page.getByTestId("terminal-panel")).toBeVisible();
      await expect.poll(() => created.length).toBe(1);
      await expect(page.getByTestId("terminal-error")).toHaveCount(0);
      const surface = page.getByTestId("terminal-surface");
      await expect(surface.locator("canvas")).toBeVisible();
      await expect(page.getByTestId("terminal-panel")).toHaveAttribute(
        "aria-busy",
        "false",
      );
      await expect(page.getByTestId("terminal-error")).toHaveCount(0);
      await surface.locator("textarea").focus();
      await page.keyboard.type(
        "printf 'native-terminal-proof' > terminal-proof.txt",
      );
      await page.keyboard.press("Enter");
      await expect
        .poll(() =>
          readFile(join(runtime.vault, "terminal-proof.txt"), "utf8").catch(
            () => "",
          ),
        )
        .toBe("native-terminal-proof");
      expect(errors).toEqual([]);
    } finally {
      await runtime.stop();
    }
  });
}
