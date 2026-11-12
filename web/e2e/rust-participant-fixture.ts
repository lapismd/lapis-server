import { expect, type Page } from "@playwright/test";
import {
  chmod,
  copyFile,
  cp,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { randomBytes } from "node:crypto";
import { join, resolve } from "node:path";
import { RustServerRuntime, unusedLoopbackPort } from "./rust-server-runtime";

/** Test-owned storage and a deterministic native peer; production UI is unchanged. */
export async function participantBrowser(page: Page) {
  const repository = resolve(import.meta.dirname, "../..");
  const root = await mkdtemp(
    join(repository, ".local/rust-participants-browser-"),
  );
  const password = randomBytes(32).toString("base64url");
  const origin = `http://localhost:${await unusedLoopbackPort()}`;
  const helper = join(root, "peer.py"),
    log = join(root, "worker.jsonl");
  await copyFile(
    join(
      repository,
      "crates/lapis-app-core/tests/support/native_participants/peer.py",
    ),
    helper,
  );
  await chmod(helper, 0o700);
  const agents = join(root, "agents.json");
  const declaration = [
    {
      id: "opaque-runtime",
      label: "Runtime display",
      default_mode: "plan",
      executable: helper,
      environment: { LOG: log, PRIVATE_NATIVE_TOKEN: "never-in-portable-chat" },
      participant_selections: ["writer", "reviewer"].map((id) => ({
        recipient: {
          id,
          label: id === "writer" ? "Writer" : "Reviewer",
          token: `jj-store-agent:${Buffer.from(id).toString("hex")};`,
        },
        target: {
          id,
          agent: "codex",
          model: { provider: "codex", model: `held-${id}` },
          thinking: "default",
        },
      })),
    },
  ];
  await writeFile(agents, JSON.stringify(declaration));
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
  const mutations: string[] = [],
    acceptances: unknown[] = [];
  page.on("request", (request) => {
    const body = request.postData();
    if (!body) return;
    if (
      request.url().endsWith("/participants") &&
      JSON.parse(body).kind === "accept"
    )
      acceptances.push(JSON.parse(body));
    if (
      request.url().includes("/commands") &&
      /transcript\.jsonl|agents\.jsonl|metadata\.yaml/u.test(body) &&
      /"kind":"(?:write_text|append|create_file)"/u.test(body)
    )
      mutations.push(body);
  });
  const chat = page.locator('[data-native-chat="true"]:visible');
  const frames = async (): Promise<Array<Record<string, unknown>>> =>
    (
      await readFile(log, "utf8").catch((error: NodeJS.ErrnoException) => {
        if (error.code === "ENOENT") return "";
        throw error;
      })
    )
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  async function open(initial = false) {
    await page.goto(origin);
    if (initial) {
      await page.getByLabel("Username").fill("owner");
      await page.getByLabel("Password").fill(password);
      await page.getByRole("button", { name: "Sign in", exact: true }).click();
      await page
        .getByRole("button", {
          name: "Open Rust vault owner · server-hosted",
          exact: true,
        })
        .click();
    }
    await expect(
      page.locator('[data-notes-session-ready="true"]'),
    ).toBeVisible();
    if (initial)
      await page
        .getByRole("button", { name: "Open Chat", exact: true })
        .click();
    await expect(
      chat
        .getByRole("group", { name: "Message composer", exact: true })
        .getByRole("textbox", { name: "Message", exact: true }),
    ).toBeEditable();
  }
  async function selectRoles() {
    for (const label of ["Writer", "Reviewer"]) {
      await chat
        .getByRole("combobox", { name: "Tag people or agents", exact: true })
        .click();
      const option = page.getByRole("option", {
        name: new RegExp(`^${label}\\b`),
      });
      await expect(option).toHaveAttribute("data-agent-selected", "false");
      await option.click();
    }
  }
  async function send(text: string, count: number) {
    await chat
      .getByRole("group", { name: "Message composer", exact: true })
      .getByRole("textbox", { name: "Message", exact: true })
      .fill(text);
    await chat
      .getByRole("button", { name: "Send message", exact: true })
      .click();
    await expect
      .poll(
        async () => (await frames()).filter((f) => f.type === "prompt").length,
      )
      .toBe(count);
  }
  async function receipt() {
    const directory = join(root, "profile/turns/participants", service.vaultId);
    const ids = (await readdir(directory)).filter((id) => id.endsWith(".json"));
    const values = await Promise.all(
      ids.map(async (id) =>
        JSON.parse(await readFile(join(directory, id), "utf8")),
      ),
    );
    return values
      .sort((a, b) =>
        String(a.input.dispatch.startedAt).localeCompare(
          String(b.input.dispatch.startedAt),
        ),
      )
      .at(-1);
  }
  const release = (role?: string) =>
    writeFile(log + ".release" + (role ? `.held-${role}` : ""), "release");
  async function checkpoint() {
    const path = join(root, "checkpoint");
    await mkdir(path);
    await cp(join(root, "profile/turns"), join(path, "turns"), {
      recursive: true,
    });
    await cp(join(service.vault, ".lapis/agents"), join(path, "agents"), {
      recursive: true,
    });
    return async () => {
      await rm(join(root, "profile/turns"), { recursive: true });
      await cp(join(path, "turns"), join(root, "profile/turns"), {
        recursive: true,
      });
      await rm(join(service.vault, ".lapis/agents"), { recursive: true });
      await cp(join(path, "agents"), join(service.vault, ".lapis/agents"), {
        recursive: true,
      });
    };
  }
  return {
    root,
    service,
    chat,
    frames,
    open,
    selectRoles,
    send,
    receipt,
    release,
    checkpoint,
    mutations,
    acceptances,
    declaration,
    agents,
    log,
  };
}
