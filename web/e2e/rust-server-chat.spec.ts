import { expect, test } from "@playwright/test";
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  writeFile,
} from "node:fs/promises";
import { randomBytes } from "node:crypto";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";

test("native chat retains sidebar History and opens the same conversation without replay", async ({
  page,
}, testInfo) => {
  const repository = resolve("../..");
  const root = await mkdtemp(join(repository, ".local/rust-chat-browser-"));
  const password = randomBytes(32).toString("base64url");
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const helper = join(root, "peer.py"),
    log = join(root, "worker.jsonl");
  await copyFile(
    join(repository, "crates/lapis-app-core/tests/support/native_turn/peer.py"),
    helper,
  );
  await chmod(helper, 0o700);
  const agents = join(root, "agents.json");
  await writeFile(
    agents,
    JSON.stringify([
      {
        id: "fixture",
        label: "Fixture agent",
        default_mode: "plan",
        executable: helper,
        environment: { LOG: log },
      },
    ]),
  );
  const service = new RustServerRuntime(
    root,
    repository,
    origin,
    password,
    agents,
  );
  await mkdir(join(service.vault, ".lapis"), { recursive: true });
  await writeFile(
    join(service.vault, ".lapis/core-plugins.json"),
    JSON.stringify({ disabled: [], enabled: ["ai"] }),
  );
  await service.start();
  const chat = page.locator('[data-native-chat="true"]');
  const history = page.getByTestId("ai-conversation-history");
  try {
    await page.goto(origin);
    await page.getByLabel("Username").fill("owner");
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await page.getByRole("button", { name: "Open Chat", exact: true }).click();
    const editor = chat.getByRole("textbox", { name: "Message", exact: true });
    await expect(editor).toBeEditable();
    await editor.fill("A native browser conversation");
    await chat
      .getByRole("button", { name: "Send message", exact: true })
      .click();
    await expect(chat).toContainText("reply café 📝");
    await chat
      .getByRole("button", { name: "Conversation actions", exact: true })
      .click();
    await page
      .getByRole("menuitem", { name: "AI conversations", exact: true })
      .click();
    await expect(history).toBeVisible();
    await expect(
      history.locator("xpath=ancestor::*[@data-workspace-surface][1]"),
    ).toHaveAttribute("data-workspace-surface", "right-sidebar");
    await history
      .getByRole("button", { name: /^A native browser conversation\b/ })
      .click();
    await expect(history).toBeVisible();
    await expect(
      chat.filter({
        has: page.getByRole("textbox", { name: "Message", exact: true }),
      }),
    ).toHaveCount(1);
    await history
      .getByRole("button", { name: "New chat in Vault root", exact: true })
      .click();
    await expect(
      page
        .getByRole("toolbar", { name: "Workspace tabs" })
        .getByRole("button", { name: "AI", exact: true }),
    ).toHaveCount(2);
    await expect(history).toBeVisible();
    await history
      .getByRole("button", { name: /^A native browser conversation\b/ })
      .click();
    await expect(
      page
        .getByRole("toolbar", { name: "Workspace tabs" })
        .getByRole("button", { name: "AI", exact: true }),
    ).toHaveCount(2);
    await expect
      .poll(async () =>
        readFile(join(service.vault, ".lapis/workspace.json"), "utf8").catch(
          (error: NodeJS.ErrnoException) => {
            if (error.code === "ENOENT") return "";
            throw error;
          },
        ),
      )
      .toContain("ai-conversation-history");
    await page.reload();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await expect(history).toBeVisible();
    await expect(
      page
        .locator('[data-native-chat="true"]')
        .filter({
          has: page.getByRole("textbox", { name: "Message", exact: true }),
        })
        .first(),
    ).toContainText("reply café 📝");
    await service.restart();
    await page.reload();
    await page
      .getByRole("button", { name: "Open Rust vault", exact: true })
      .click();
    await expect(history).toBeVisible();
    await history
      .getByRole("button", { name: /^A native browser conversation\b/ })
      .click();
    await expect(
      page
        .getByRole("toolbar", { name: "Workspace tabs" })
        .getByRole("button", { name: "AI", exact: true }),
    ).toHaveCount(2);
    const frames = (await readFile(log, "utf8"))
      .split("\n")
      .filter(Boolean)
      .map((s) => JSON.parse(s));
    expect(frames.filter((s) => s.type === "prompt")).toHaveLength(1);
    expect(
      frames.filter((s) => s.type === "configure").map((s) => s.mode),
    ).toEqual(["plan"]);
  } catch (error) {
    await testInfo.attach("Rust chat host log", {
      body: service.logs,
      contentType: "text/plain",
    });
    throw error;
  } finally {
    await service.stop();
  }
});
