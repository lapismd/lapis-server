import { expect, test } from "@playwright/test";
import { randomBytes } from "node:crypto";
import { mkdtemp, mkdir, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";
import { signInHostedOwner } from "./rust-hosted-session";

test("enroll a passkey, sign in with it after reload, and remove it", async ({
  page,
}) => {
  const repository = resolve(import.meta.dirname, "../..");
  await mkdir(join(repository, ".local"), { recursive: true });
  const root = await mkdtemp(join(repository, ".local/passkeys-"));
  const password = randomBytes(32).toString("base64url");
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const server = new RustServerRuntime(
    root,
    repository,
    origin,
    password,
    undefined,
    undefined,
    join(repository, "web/dist"),
  );
  const devtools = await page.context().newCDPSession(page);
  await devtools.send("WebAuthn.enable");
  await devtools.send("WebAuthn.addVirtualAuthenticator", {
    options: {
      protocol: "ctap2",
      transport: "internal",
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });
  try {
    await server.start();
    await page.goto(origin);
    await signInHostedOwner(page, password);
    await page.getByRole("button", { name: "Account & security" }).click();
    await page.getByLabel("New passkey name").fill("Local authenticator");
    await page
      .getByRole("button", { name: "Add passkey", exact: true })
      .click();
    await expect(
      page.getByText("Passkey added.", { exact: true }),
    ).toBeVisible();
    await page.getByRole("dialog").press("Escape");
    await page.getByRole("button", { name: "Sign out", exact: true }).click();
    await page.reload();
    await page.getByLabel("Username").fill("owner");
    await page.getByRole("button", { name: "Sign in with a passkey" }).click();
    await expect(
      page.getByRole("heading", { name: "Your vaults" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Account & security" }).click();
    await expect(page.getByText(/Last used/)).toBeVisible();
    await page
      .getByRole("button", { name: "Remove Local authenticator" })
      .click();
    await expect(
      page.getByText("Passkey removed.", { exact: true }),
    ).toBeVisible();
    await expect(page.getByText("No passkeys enrolled.")).toBeVisible();
    await page.getByRole("dialog").press("Escape");
    await page.getByRole("button", { name: "Sign out", exact: true }).click();
    await signInHostedOwner(page, password);
  } finally {
    await server.stop();
    await rm(root, { recursive: true, force: true });
  }
});

test("Swagger UI calls the hosted API with the owner session", async ({
  page,
}) => {
  const repository = resolve(import.meta.dirname, "../..");
  const root = await mkdtemp(join(repository, ".local/swagger-ui-"));
  const password = randomBytes(32).toString("base64url");
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const server = new RustServerRuntime(
    root,
    repository,
    origin,
    password,
    undefined,
    undefined,
    join(repository, "web/dist"),
  );
  try {
    await server.start();
    await page.goto(origin);
    await signInHostedOwner(page, password);
    await page.goto(`${origin}/api/docs`);
    await expect(
      page.getByRole("heading", { name: "Lapis Server HTTP API" }),
    ).toBeVisible();
    const operation = page
      .locator(".opblock")
      .filter({ hasText: "Read the current browser session" });
    await operation.getByRole("button").first().click();
    await operation.getByRole("button", { name: "Try it out" }).click();
    const response = page.waitForResponse(
      (result) => result.url() === `${origin}/api/v3/auth/current`,
    );
    await operation.getByRole("button", { name: "Execute" }).click();
    const result = await response;
    expect(result.status()).toBe(200);
    expect(result.request().headers()["x-lapis-protocol"]).toBe("3");
    expect((await result.json()).username).toBe("owner");
  } finally {
    await server.stop();
    await rm(root, { recursive: true, force: true });
  }
});
