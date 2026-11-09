import { afterEach, expect, it, vi } from "vitest";
import { WorkspaceSettingsController } from "@lapismd/design-core/workspace/settings";
import { contentFixture } from "../../../stories/fixtures/content-access";
import { RustHttpClient } from "../hosted/rust/client";
import { ContentAccessModel } from "./model";
import { definition } from "./definition";

function fixture(intercept?: typeof fetch) {
  const source = contentFixture(() => [
    { id: "notes", name: "Notes", role: "owner" },
  ]);
  const transport = vi.fn<typeof fetch>(async (url, init) => {
    if (String(url).endsWith("/auth/current"))
      return Response.json({ csrf_token: "owner-csrf" });
    if (intercept && String(url).endsWith("/grants") && init?.method === "POST")
      return intercept(url, init);
    return source.fetch(url, init);
  });
  const host = new RustHttpClient("https://notes.example", transport);
  return { model: new ContentAccessModel(host, "notes"), host, transport };
}
afterEach(() => vi.useRealTimers());

it("uses owner CSRF and keeps one-time tokens out of controller persistence", async () => {
  const { model, host, transport } = fixture();
  const settings = new WorkspaceSettingsController();
  const unregister = settings.registerDefinition(definition(model));
  try {
    await host.current();
    await model.refresh();
    expect(await settings.set("content-access.enabled", true)).toBe(true);
    await model.create();
    const token = model.get("content-access.token");
    expect(token).toMatch(/^lc1_/u);
    expect(JSON.stringify(settings.getSnapshot())).not.toContain(token);
    expect(settings.getSnapshot().values).toEqual({});
    const issue = transport.mock.calls.find(
      ([url, init]) =>
        String(url).endsWith("/grants") && init?.method === "POST",
    )!;
    expect(new Headers(issue[1]?.headers).get("x-lapis-csrf")).toBe(
      "owner-csrf",
    );
    expect(JSON.parse(String(issue[1]?.body))).toEqual({
      label: "CLI client",
      vault_ids: ["notes"],
      lifetime_seconds: 3600,
    });
    expect(settings.get("content-access.token")).toBe(token);
    model.dispose();
    expect(settings.get("content-access.token")).toBe("");
  } finally {
    model.dispose();
    unregister();
    settings.destroy();
  }
});

it("clears credentials on dismissal, timeout and disable without persisting form edits", async () => {
  const { model } = fixture();
  try {
    await model.refresh();
    await model.set("content-access.enabled", true);
    await model.set("content-access.label", "Script");
    await model.set("content-access.hours", 24);
    await model.create();
    expect(model.hasSecret).toBe(true);
    model.clearSecret();
    expect(model.hasSecret).toBe(false);
    vi.useFakeTimers();
    await model.create();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(model.hasSecret).toBe(false);
    await model.create();
    await model.set("content-access.enabled", false);
    expect(model.hasSecret).toBe(false);
    expect(model.get("content-access.grants")).toEqual([]);
    expect(JSON.stringify(model)).not.toContain("lc1_");
  } finally {
    model.dispose();
  }
});

it("aborts on disposal and discards an issuance response that arrives afterward", async () => {
  let respond!: (response: Response) => void;
  let started!: () => void;
  let signal: AbortSignal | null | undefined;
  const pending = new Promise<Response>((resolve) => {
    respond = resolve;
  });
  const requested = new Promise<void>((resolve) => {
    started = resolve;
  });
  const { model } = fixture(async (_url, init) => {
    signal = init?.signal;
    started();
    return pending;
  });
  await model.refresh();
  await model.set("content-access.enabled", true);
  const listener = vi.fn();
  model.subscribe(listener);
  const issuing = model.create();
  await requested;
  model.dispose();
  const notifications = listener.mock.calls.length;
  expect(signal?.aborted).toBe(true);
  respond(Response.json({ grant_id: "late", token: "late-secret" }));
  await issuing;
  expect(model.hasSecret).toBe(false);
  expect(listener).toHaveBeenCalledTimes(notifications);
});

it("does not replay a failed issuance or disclose malformed response content", async () => {
  const secret = "response-content-must-not-appear";
  const { model, transport } = fixture(
    async () => new Response(`{"token":"${secret}`),
  );
  try {
    await model.refresh();
    await model.set("content-access.enabled", true);
    await expect(model.create()).rejects.toThrow(
      "Content request outcome is unavailable. Refresh status before retrying.",
    );
    expect(model.notice).not.toContain(secret);
    expect(model.hasSecret).toBe(false);
    expect(
      transport.mock.calls.filter(([url]) => String(url).endsWith("/grants")),
    ).toHaveLength(1);
  } finally {
    model.dispose();
  }
});

it("allows replacing a draft label and preserves spaces while typing", async () => {
  const { model, transport } = fixture();
  const settings = new WorkspaceSettingsController();
  const unregister = settings.registerDefinition(definition(model));
  try {
    await model.refresh();
    await model.set("content-access.enabled", true);
    expect(await settings.set("content-access.label", "")).toBe(true);
    expect(await settings.set("content-access.label", "Story ")).toBe(true);
    expect(settings.get("content-access.label")).toBe("Story ");
    expect(await settings.set("content-access.label", "Story client")).toBe(
      true,
    );
    await model.create();
    const issue = transport.mock.calls.find(([url]) =>
      String(url).endsWith("/grants"),
    )!;
    expect(JSON.parse(String(issue[1]?.body)).label).toBe("Story client");
  } finally {
    model.dispose();
    unregister();
    settings.destroy();
  }
});
