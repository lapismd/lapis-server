import { expect, test } from "@playwright/test";
import { createHash, randomBytes, randomUUID } from "node:crypto";
import { mkdir, mkdtemp } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";

test.use({ trace: "retain-on-failure" });

test("compiled server consent survives restart and grants only revocable discovery", async ({
  page,
  playwright,
}) => {
  const repository = resolve(import.meta.dirname, "../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  const root = await mkdtemp(join(repository, ".local/sync-consent-browser-"));
  const password = randomBytes(32).toString("base64url");
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const runtime = new RustServerRuntime(
    root,
    repository,
    origin,
    password,
    undefined,
    undefined,
    join(repository, "web/dist"),
  );
  const native = await playwright.request.newContext({
    extraHTTPHeaders: { "x-lapis-protocol": "3" },
  });
  const verifier = randomBytes(32).toString("base64url");
  const errors: string[] = [];
  page.on("pageerror", (error) => {
    errors.push(error.message);
    console.info(
      "consent page error:",
      error.message
        .replaceAll(password, "[fixture password]")
        .replaceAll(verifier, "[fixture verifier]"),
    );
  });
  try {
    await runtime.start();
    const initiated = await native.post(`${origin}/api/v3/sync/account`, {
      data: {
        client_id: randomUUID(),
        client_name: "Independent Desktop",
        s256_challenge: createHash("sha256")
          .update(verifier)
          .digest("base64url"),
      },
    });
    expect(initiated.status()).toBe(200);
    const connection = await initiated.json();
    expect(connection.approval_url).toBe(
      `${origin}/?sync-connect=${connection.id}`,
    );
    await runtime.stop();
    await runtime.start();
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(connection.approval_url);
    console.info(
      "consent initial page:",
      await page.locator("body").ariaSnapshot(),
    );
    await expect(page.getByLabel("Username")).toBeVisible();
    await page.getByLabel("Username").fill("owner");
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await expect(
      page.getByRole("heading", { name: "Independent Desktop" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Approve connection" }),
    ).toBeEnabled();
    await page.getByRole("button", { name: "Reject", exact: true }).focus();
    await page.keyboard.press("Tab");
    await expect(
      page.getByRole("button", { name: "Approve connection" }),
    ).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("status")).toContainText("Connection approved");
    const redeemed = await native.post(
      `${origin}/api/v3/sync/account/${connection.id}/redeem`,
      { data: { verifier } },
    );
    expect(redeemed.status()).toBe(200);
    const issued = await redeemed.json();
    const bearer = { authorization: `Bearer ${issued.token}` };
    expect(
      (
        await native.get(`${origin}/api/v3/vaults`, { headers: bearer })
      ).status(),
    ).toBe(401);
    expect(
      (
        await native.get(`${origin}/api/v3/sync/discovery/vaults`, {
          headers: bearer,
        })
      ).status(),
    ).toBe(200);
    const storage = await page.evaluate(() =>
      JSON.stringify([localStorage, sessionStorage]),
    );
    expect(storage).not.toContain(issued.token);
    expect(storage).not.toContain(verifier);
    await page.reload();
    await expect(page.getByRole("status")).toContainText("Connection approved");
    await runtime.stop();
    await runtime.start();
    expect(
      (
        await native.post(`${origin}/api/v3/sync/discovery/sign-out`, {
          headers: bearer,
        })
      ).status(),
    ).toBe(200);
    expect(
      (
        await native.get(`${origin}/api/v3/sync/discovery/vaults`, {
          headers: bearer,
        })
      ).status(),
    ).toBe(401);
    expect(errors).toEqual([]);
  } finally {
    await native.dispose();
    await runtime.stop();
  }
});
