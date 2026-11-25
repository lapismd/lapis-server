import { expect, test } from "@playwright/test";
import { parse } from "yaml";
import { readFile, writeFile, readdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { JobSnapshot } from "../../crates/lapis-app-protocol/bindings/protocol";
import { buildTasksArtifact } from "./tasks-installed-plugin";
import { createHostedTasksFixture } from "./rust-tasks-fixture";
import { signOutHostedWorkspace } from "./rust-hosted-session";

let artifact: string;
test.beforeAll(async () => {
  artifact = await buildTasksArtifact(resolve(import.meta.dirname, "../.."));
  const manifest = JSON.parse(
    await readFile(join(artifact, "manifest.json"), "utf8"),
  );
  manifest.lapis.contributes = {
    backgroundJobs: [
      {
        id: "refresh",
        title: "Background refresh",
        description: "Native host acceptance",
        entry: "job.js",
        schedule: { kind: "interval", seconds: 86400 },
        permissions: {
          metadataRead: false,
          frontmatterRead: true,
          frontmatterFields: ["jobCount"],
          notifications: false,
        },
      },
    ],
  };
  await writeFile(join(artifact, "manifest.json"), JSON.stringify(manifest));
  await writeFile(
    join(artifact, "job.js"),
    `export function run(ctx) {
    const count = (ctx.state.load()?.count ?? 0) + 1;
    const until = Date.now() + 5000;
    if (ctx.signal.aborted) throw Error("Cancelled");
    while (Date.now() < until) {}
    const note = ctx.notes.frontmatter("Note.md");
    ctx.notes.patchFrontmatter({path:"Note.md", expectedVersion:note.version, set:{jobCount:count}});
    ctx.state.save({count});
  }`,
  );
});

test("installed background jobs run after browser detach and restore private checkpoints on cold restart", async ({
  page,
}, info) => {
  const f = await createHostedTasksFixture(page, artifact);
  async function jobs(command: Record<string, unknown>): Promise<JobSnapshot> {
    const reply = await f.execute({
      kind: "plugin_jobs",
      command,
    } as Parameters<typeof f.execute>[0]);
    expect(reply.outcome.status).toBe("ok");
    expect(reply.outcome.value.kind).toBe("plugin_jobs");
    return reply.outcome.value.snapshot;
  }
  const control = { pluginId: "tasks", jobId: "refresh" };
  const text = () => readFile(join(f.service.vault, "Note.md"), "utf8");
  const savedCount = async () => {
    const saved = await text();
    const yaml = /^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/u.exec(saved)?.[1];
    return yaml ? parse(yaml).jobCount : undefined;
  };
  try {
    await f.open({ tasks: false });
    const declared = await jobs({ kind: "list" });
    expect(declared.jobs).toHaveLength(1);
    expect(declared.jobs[0]!.configuration).toBeNull();
    await page
      .getByRole("button", { name: "Open settings", exact: true })
      .click();
    const settings = page.getByRole("dialog", {
      name: "Settings",
      exact: true,
    });
    await settings
      .getByRole("button", { name: "Background jobs", exact: true })
      .click();
    await expect(
      settings.getByRole("heading", {
        name: "Background refresh",
        exact: true,
      }),
    ).toBeVisible();
    await settings
      .getByRole("button", { name: "Approve and enable", exact: true })
      .click();
    await expect(
      settings.getByRole("button", { name: "Run now", exact: true }),
    ).toBeEnabled();
    await settings
      .getByRole("button", { name: "Run now", exact: true })
      .click();
    await expect
      .poll(
        async () => (await jobs({ kind: "list" })).jobs[0]!.runs.at(-1)?.phase,
      )
      .toBe("running");
    await signOutHostedWorkspace(page);
    await expect.poll(savedCount).toBe(1);
    expect(await text()).toContain("# Native hosted note");
    await f.open({ tasks: false });
    await expect
      .poll(
        async () => (await jobs({ kind: "list" })).jobs[0]!.runs.at(-1)?.phase,
      )
      .toBe("completed");
    await f.restart();
    await f.open({ tasks: false });
    expect((await jobs({ kind: "list" })).jobs[0]!.configuration?.enabled).toBe(
      true,
    );
    await jobs({ kind: "runNow", ...control });
    await expect.poll(savedCount).toBe(2);
    await expect
      .poll(
        async () => (await jobs({ kind: "list" })).jobs[0]!.runs.at(-1)?.phase,
      )
      .toBe("completed");
    await jobs({ kind: "pause", ...control });
    expect((await jobs({ kind: "list" })).jobs[0]!.configuration?.enabled).toBe(
      false,
    );
  } finally {
    if (info.status !== info.expectedStatus) {
      const directory = join(
        f.service.root,
        "profile/plugin-jobs",
        f.service.vaultId,
      );
      for (const name of await readdir(directory).catch(() => [])) {
        if (name.endsWith(".json"))
          console.log(
            "BACKGROUND_JOB_FAILURE",
            await readFile(join(directory, name), "utf8"),
          );
      }
    }
    await f.stop(info);
  }
});
